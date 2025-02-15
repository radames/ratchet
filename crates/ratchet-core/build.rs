use anyhow::Context as anyhowCtx;

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use strum::IntoEnumIterator;
use tera::{Context, Tera};

#[derive(strum_macros::EnumIter, Debug)]
pub enum KernelElement {
    Scalar,
    Vec2,
    Vec4,
}

impl std::fmt::Display for KernelElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            KernelElement::Scalar => "scalar",
            KernelElement::Vec2 => "vec2",
            KernelElement::Vec4 => "vec4",
        };
        write!(f, "{}", s)
    }
}

pub enum WgslDType {
    F32,
}

impl std::fmt::Display for WgslDType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WgslDType::F32 => write!(f, "f32"),
        }
    }
}

impl KernelElement {
    pub fn as_wgsl(&self, dtype: WgslDType) -> String {
        match self {
            KernelElement::Scalar => dtype.to_string(),
            KernelElement::Vec2 => format!("vec2<{}>", dtype),
            KernelElement::Vec4 => format!("vec4<{}>", dtype),
        }
    }

    pub fn as_size(&self) -> usize {
        match self {
            KernelElement::Scalar => 1,
            KernelElement::Vec2 => 2,
            KernelElement::Vec4 => 4,
        }
    }
}

#[derive(Debug, Clone, strum_macros::EnumIter)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinaryOp {
    pub fn mapping(&self) -> (&'static str, &'static str) {
        match self {
            BinaryOp::Add => ("add", "+"),
            BinaryOp::Sub => ("sub", "-"),
            BinaryOp::Mul => ("mul", "*"),
            BinaryOp::Div => ("div", "/"),
        }
    }
}

#[derive(Debug, Clone, strum_macros::EnumIter)]
pub enum UnaryOp {
    Gelu,
    Tanh,
    Exp,
    Log,
    Sin,
    Cos,
    Abs,
    Sqrt,
    Relu,
    Floor,
    Ceil,
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            UnaryOp::Gelu => "gelu",
            UnaryOp::Tanh => "tanh",
            UnaryOp::Exp => "exp",
            UnaryOp::Log => "log",
            UnaryOp::Sin => "sin",
            UnaryOp::Cos => "cos",
            UnaryOp::Abs => "abs",
            UnaryOp::Sqrt => "sqrt",
            UnaryOp::Relu => "relu",
            UnaryOp::Floor => "floor",
            UnaryOp::Ceil => "ceil",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, strum_macros::EnumIter)]
pub enum ReindexOp {
    Permute,
    Slice,
    Broadcast,
}

impl std::fmt::Display for ReindexOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ReindexOp::Permute => "permute",
            ReindexOp::Slice => "slice",
            ReindexOp::Broadcast => "broadcast",
        };
        write!(f, "{}", s)
    }
}

impl ReindexOp {
    pub fn func_body(&self) -> String {
        match self {
            ReindexOp::Permute => r#"
    var src_index = vec4<u32>(0u);
    src_index[metadata.perm[0]] = dst_index[0]; 
    src_index[metadata.perm[1]] = dst_index[1];
    src_index[metadata.perm[2]] = dst_index[2];
    src_index[metadata.perm[3]] = dst_index[3];"#
                .to_string(),
            ReindexOp::Slice => r#"
    var src_index = dst_index;"#
                .to_string(),
            ReindexOp::Broadcast => r#"
    var src_index = select(dst_index, vec4<u32>(0u), metadata.src_shape == vec4<u32>(1u));
    "#
            .to_string(),
        }
    }
}

#[derive(Debug, Clone, strum_macros::EnumIter)]
pub enum NormOp {
    LayerNorm,
}

impl std::fmt::Display for NormOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NormOp::LayerNorm => "layernorm",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug)]
pub struct KernelGenerator {
    tera: Tera,
    dest_path: PathBuf,
    templates_path: PathBuf,
}

impl Default for KernelGenerator {
    fn default() -> Self {
        let base_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        KernelGenerator {
            tera: Tera::default(),
            dest_path: base_path.join("kernels").join("generated"),
            templates_path: base_path.join("kernel-templates"),
        }
    }
}

impl KernelGenerator {
    fn generate(&mut self) -> anyhow::Result<()> {
        self.generate_unary()?;
        self.generate_binary()?;
        self.generate_reindex()?;
        self.generate_norm()?;
        Ok(())
    }

    fn generate_reindex(&mut self) -> anyhow::Result<()> {
        for op in ReindexOp::iter() {
            let path = self.templates_path.join("reindex.wgsl");
            self.tera.add_template_file(path, Some("reindex"))?;

            let mut context = Context::new();
            context.insert("func_body", &op.func_body());
            let rendered = self.tera.render("reindex", &context)?;

            let kernel_fname = format!("{}_{}.wgsl", op, KernelElement::Scalar);
            let mut file = File::create(self.dest_path.join(kernel_fname))?;
            file.write_all(rendered.as_bytes())?;
        }
        Ok(())
    }

    fn generate_norm(&mut self) -> anyhow::Result<()> {
        for op in NormOp::iter() {
            for ke in KernelElement::iter() {
                let path = self.templates_path.join("layernorm.wgsl");
                self.tera.add_template_file(path, Some("layernorm"))?;

                let mut context = Context::new();
                context.insert("elem", &ke.as_wgsl(WgslDType::F32));
                context.insert("elem_size", &ke.as_size());
                let reduction_len = match ke {
                    KernelElement::Scalar => "metadata.N",
                    KernelElement::Vec2 => "metadata.ND2",
                    KernelElement::Vec4 => "metadata.ND4",
                };
                context.insert("reduction_len", reduction_len);
                let rendered = self.tera.render("layernorm", &context)?;

                let kernel_fname = format!("{}_{}.wgsl", op, ke);
                let mut file = File::create(self.dest_path.join(kernel_fname))?;
                file.write_all(rendered.as_bytes())?;
            }
        }
        Ok(())
    }

    fn generate_unary(&mut self) -> anyhow::Result<()> {
        for func in UnaryOp::iter() {
            for ke in KernelElement::iter() {
                let path = self.templates_path.join("unary.wgsl");
                self.tera.add_template_file(path, Some("unary"))?;

                let mut context = Context::new();
                let tera_func = match func {
                    UnaryOp::Tanh => String::from("safe_tanh"),
                    _ => func.to_string(),
                };
                context.insert("func", &tera_func);
                context.insert("elem", &ke.as_wgsl(WgslDType::F32));
                context.insert("elem_size", &ke.as_size());
                let rendered = self.tera.render("unary", &context)?;

                let kernel_fname = format!("{}_{}.wgsl", func, ke);
                let mut file = File::create(self.dest_path.join(kernel_fname))?;
                file.write_all(rendered.as_bytes())?;
            }
        }
        Ok(())
    }

    fn generate_binary(&mut self) -> anyhow::Result<()> {
        let pairs = BinaryOp::iter().fold(Vec::new(), |mut acc, op| {
            acc.push(op.mapping());
            acc
        });

        for (op_name, op) in &pairs {
            for ke in KernelElement::iter() {
                let path = self.templates_path.join("binary.wgsl");
                self.tera.add_template_file(path, Some("binary"))?;

                let mut context = Context::new();
                context.insert("op", op);
                context.insert("elem", &ke.as_wgsl(WgslDType::F32));
                context.insert("elem_size", &ke.as_size());
                let rendered = self.tera.render("binary", &context)?;

                let kernel_fname = format!("{}_{}.wgsl", op_name, ke);
                let mut file = File::create(self.dest_path.join(kernel_fname))?;
                file.write_all(rendered.as_bytes())?;
            }
        }
        Ok(())
    }
}

fn embed_kernels() -> anyhow::Result<()> {
    let out_dir = env!("CARGO_MANIFEST_DIR").to_string() + "/src";
    let mut file = std::fs::File::create(Path::new(&out_dir).join("kernels.rs")).context(
        "Failed to create `src/kernels.rs`. Make sure you have `src` directory in your project.",
    )?;
    writeln!(
        &file,
        "// This file is generated by build.rs. Do not edit it manually."
    )?;
    writeln!(&mut file, "use std::collections::HashMap;")?;
    writeln!(&mut file, "use lazy_static::lazy_static;")?;
    writeln!(&mut file, "lazy_static! {{")?;
    writeln!(
        &mut file,
        "pub static ref KERNELS: HashMap<&'static str, &'static str> = {{"
    )?;
    writeln!(&mut file, "    let mut m = HashMap::new();")?;
    for entry in
        globwalk::glob(env!("CARGO_MANIFEST_DIR").to_string() + "/kernels/**.wgsl")?.flatten()
    {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_str().unwrap();

        let diff = pathdiff::diff_paths(path, Path::new(out_dir.as_str()))
            .ok_or(anyhow::format_err!("Failed to get path diff"))?;

        writeln!(
            &mut file,
            "    m.insert(\"{}\", include_str!(r\"{}\"));",
            name,
            diff.display()
        )?;
    }
    writeln!(&mut file, "    m")?;
    writeln!(&mut file, "}};")?;
    writeln!(&mut file, "}}")?;

    Ok(())
}

fn main() {
    let mut generator = KernelGenerator::default();
    generator.generate().unwrap();
    embed_kernels().unwrap();
    if let Err(e) = Command::new("cargo").args(["fmt"]).status() {
        eprintln!("Failed to execute `cargo fmt`: {}", e);
    }
}
