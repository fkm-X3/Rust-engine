use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=shaders/");

    let shader_dir = Path::new("shaders");
    if !shader_dir.exists() {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("shaders");
    std::fs::create_dir_all(&out_path).unwrap();

    let compiler = shaderc::Compiler::new().expect("Failed to create shader compiler");
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_3 as u32,
    );

    for entry in std::fs::read_dir(shader_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let kind = match ext {
            "vert" => shaderc::ShaderKind::Vertex,
            "frag" => shaderc::ShaderKind::Fragment,
            "comp" => shaderc::ShaderKind::Compute,
            "geom" => shaderc::ShaderKind::Geometry,
            _ => continue,
        };

        let source = std::fs::read_to_string(&path).unwrap();
        let filename = path.file_name().unwrap().to_str().unwrap();

        println!("cargo:rerun-if-changed=shaders/{}", filename);

        let artifact = compiler
            .compile_into_spirv(&source, kind, filename, "main", Some(&options))
            .unwrap_or_else(|e| panic!("Failed to compile shader {}: {}", filename, e));

        let spv_name = format!("{}.spv", filename);
        std::fs::write(out_path.join(&spv_name), artifact.as_binary_u8()).unwrap();

        println!("cargo:warning=Compiled shader: {} -> {}", filename, spv_name);
    }
}
