use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use uuid::Uuid;

// ─── Handle ───────────────────────────────────────────────────────────────────

/// A strongly-typed, reference-counted handle to a loaded asset.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetHandle {
    id:  Uuid,
    kind: &'static str,
}

impl AssetHandle {
    fn new(kind: &'static str) -> Self {
        Self { id: Uuid::new_v4(), kind }
    }

    pub fn id(&self) -> Uuid { self.id }
    pub fn kind(&self) -> &str { self.kind }
}

// ─── Load state ───────────────────────────────────────────────────────────────

/// The loading state of an asset.
#[derive(Debug)]
pub enum AssetState<T> {
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> AssetState<T> {
    pub fn is_ready(&self) -> bool {
        matches!(self, AssetState::Ready(_))
    }

    pub fn get(&self) -> Option<&T> {
        match self {
            AssetState::Ready(v) => Some(v),
            _ => None,
        }
    }
}

// ─── Texture ──────────────────────────────────────────────────────────────────

/// A CPU-side texture (width, height, RGBA pixels).
#[derive(Debug, Clone)]
pub struct Texture {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>,    // RGBA8 linear
    pub format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Bc3Srgb,    // DXT5
}

impl Texture {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let img = image::open(path)?.to_rgba8();
        Ok(Self {
            width:  img.width(),
            height: img.height(),
            pixels: img.into_raw(),
            format: TextureFormat::Rgba8Srgb,
        })
    }

    /// 1×1 solid-colour fallback texture.
    pub fn solid(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            width: 1,
            height: 1,
            pixels: vec![r, g, b, a],
            format: TextureFormat::Rgba8Srgb,
        }
    }
}

// ─── Audio ────────────────────────────────────────────────────────────────────

/// A loaded audio clip (raw samples + metadata).
#[derive(Debug, Clone)]
pub struct AudioClip {
    pub path:        PathBuf,
    pub sample_rate: u32,
    pub channels:    u16,
    pub duration:    f32,
}

impl AudioClip {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        // rodio's Decoder needs the raw file; we just record metadata here
        let path = path.as_ref().to_owned();
        // In a full impl: decode, store samples.
        Ok(Self {
            sample_rate: 44100,
            channels: 2,
            duration: 0.0, // computed from decode
            path,
        })
    }
}

// ─── Asset Manager ────────────────────────────────────────────────────────────

// Future expansion: generic asset storage
#[allow(dead_code)]
type AnyMap = HashMap<Uuid, Box<dyn Any + Send + Sync>>;

/// Central registry for all game assets.
pub struct AssetManager {
    root:     PathBuf,
    textures: HashMap<Uuid, AssetState<Texture>>,
    audio:    HashMap<Uuid, AssetState<AudioClip>>,
    /// Path → handle cache (avoid duplicate loads)
    path_cache: HashMap<PathBuf, AssetHandle>,
    /// Background-load sender
    load_tx: std::sync::mpsc::Sender<LoadRequest>,
    load_rx: std::sync::mpsc::Receiver<LoadResult>,
}

enum LoadRequest {
    Texture { id: Uuid, path: PathBuf },
    Audio   { id: Uuid, path: PathBuf },
}

enum LoadResult {
    Texture { id: Uuid, result: Result<Texture> },
    Audio   { id: Uuid, result: Result<AudioClip> },
}

impl AssetManager {
    pub fn new() -> Self {
        let (load_tx, worker_rx) = std::sync::mpsc::channel::<LoadRequest>();
        let (worker_tx, load_rx) = std::sync::mpsc::channel::<LoadResult>();

        // Background loader thread
        std::thread::Builder::new()
            .name("asset-loader".into())
            .spawn(move || {
                while let Ok(req) = worker_rx.recv() {
                    match req {
                        LoadRequest::Texture { id, path } => {
                            let result = Texture::load(&path);
                            let _ = worker_tx.send(LoadResult::Texture { id, result });
                        }
                        LoadRequest::Audio { id, path } => {
                            let result = AudioClip::load(&path);
                            let _ = worker_tx.send(LoadResult::Audio { id, result });
                        }
                    }
                }
            })
            .expect("Failed to spawn asset loader thread");

        Self {
            root: PathBuf::from("assets"),
            textures: HashMap::new(),
            audio:    HashMap::new(),
            path_cache: HashMap::new(),
            load_tx,
            load_rx,
        }
    }

    /// Set the root asset directory.
    pub fn set_root<P: Into<PathBuf>>(&mut self, root: P) {
        self.root = root.into();
    }

    // ─── Texture ──────────────────────────────────────────────────────────────

    /// Begin loading a texture asynchronously.
    pub fn load_texture<P: AsRef<Path>>(&mut self, path: P) -> AssetHandle {
        let full = self.root.join(path.as_ref());

        if let Some(h) = self.path_cache.get(&full) {
            return h.clone();
        }

        let handle = AssetHandle::new("texture");
        self.textures.insert(handle.id, AssetState::Loading);
        self.path_cache.insert(full.clone(), handle.clone());

        let _ = self.load_tx.send(LoadRequest::Texture {
            id: handle.id,
            path: full,
        });

        handle
    }

    /// Load a texture synchronously (blocks caller).
    pub fn load_texture_sync<P: AsRef<Path>>(&mut self, path: P) -> Result<AssetHandle> {
        let full = self.root.join(path.as_ref());
        if let Some(h) = self.path_cache.get(&full) {
            return Ok(h.clone());
        }
        let tex = Texture::load(&full)?;
        let handle = AssetHandle::new("texture");
        self.textures.insert(handle.id, AssetState::Ready(tex));
        self.path_cache.insert(full, handle.clone());
        Ok(handle)
    }

    /// Get a texture by handle (returns None if still loading).
    pub fn texture(&self, handle: &AssetHandle) -> Option<&Texture> {
        self.textures.get(&handle.id)?.get()
    }

    /// Instantly register a CPU-side texture (e.g. procedural).
    pub fn register_texture(&mut self, tex: Texture) -> AssetHandle {
        let handle = AssetHandle::new("texture");
        self.textures.insert(handle.id, AssetState::Ready(tex));
        handle
    }

    // ─── Audio ────────────────────────────────────────────────────────────────

    pub fn load_audio<P: AsRef<Path>>(&mut self, path: P) -> AssetHandle {
        let full = self.root.join(path.as_ref());
        let handle = AssetHandle::new("audio");
        self.audio.insert(handle.id, AssetState::Loading);
        let _ = self.load_tx.send(LoadRequest::Audio { id: handle.id, path: full });
        handle
    }

    pub fn audio(&self, handle: &AssetHandle) -> Option<&AudioClip> {
        self.audio.get(&handle.id)?.get()
    }

    // ─── Tick ─────────────────────────────────────────────────────────────────

    /// Poll background results; call once per frame.
    pub fn tick(&mut self) {
        while let Ok(result) = self.load_rx.try_recv() {
            match result {
                LoadResult::Texture { id, result } => {
                    let state = match result {
                        Ok(tex) => {
                            log::info!("Texture loaded: {:?}", id);
                            AssetState::Ready(tex)
                        }
                        Err(e) => {
                            log::error!("Texture load failed {:?}: {}", id, e);
                            AssetState::Failed(e.to_string())
                        }
                    };
                    self.textures.insert(id, state);
                }
                LoadResult::Audio { id, result } => {
                    let state = match result {
                        Ok(clip) => AssetState::Ready(clip),
                        Err(e) => AssetState::Failed(e.to_string()),
                    };
                    self.audio.insert(id, state);
                }
            }
        }
    }

    // ─── Stats ────────────────────────────────────────────────────────────────

    pub fn texture_count(&self) -> usize { self.textures.len() }
    pub fn audio_count(&self)   -> usize { self.audio.len() }
    pub fn total_assets(&self)  -> usize { self.texture_count() + self.audio_count() }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}