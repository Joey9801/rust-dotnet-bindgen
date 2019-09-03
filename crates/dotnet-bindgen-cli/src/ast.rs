use std::io;

use heck::{CamelCase, MixedCase};

use dotnet_bindgen_core::*;

static INDENT_TOK: &'static str = "    ";

fn render_indent(f: &mut dyn io::Write, ctx: &RenderContext) -> Result<(), io::Error> {
    for _ in 0..ctx.indent_level {
        write!(f, "{}", INDENT_TOK)?;
    }

    Ok(())
}

macro_rules! render_ln {
    ($f:ident, &$ctx:ident, $($args:expr),+) => {
        {
            let mut result = render_indent($f, &$ctx);

            if result.is_ok() {
                result = write!($f, $($args),+);
            }

            if result.is_ok() {
                result = write!($f, "\n");
            }
            result
        }
    }
}

#[derive(Clone, Default)]
pub struct RenderContext {
    indent_level: u8,
}

impl RenderContext {
    fn indented(&self) -> Self {
        RenderContext {
            indent_level: self.indent_level + 1,
            ..*self
        }
    }
}

pub trait AstNode {
    fn render(&self, f: &mut dyn io::Write, ctx: RenderContext) -> Result<(), io::Error>;
}

impl AstNode for FfiType {
    fn render(&self, f: &mut dyn io::Write, _ctx: RenderContext) -> Result<(), io::Error> {
        match self {
            FfiType::Int { width, signed } => {
                match width {
                    8 => {
                        if *signed {
                            write!(f, "SByte")?;
                        } else {
                            write!(f, "Byte")?;
                        }
                    }
                    16 | 32 | 64 => {
                        let base = if *signed { "Int" } else { "UInt" };
                        write!(f, "{}{}", base, width)?;
                    }
                    // TODO: technically not unreachable, should return a sensible error.
                    _ => unreachable!(),
                }
            }
            FfiType::Void => write!(f, "void")?,
        };

        Ok(())
    }
}

pub struct Root {
    pub file_comment: Option<BlockComment>,
    pub using_statements: Vec<UsingStatement>,
    pub children: Vec<Box<dyn AstNode>>,
}

impl Root {
    pub fn render(&self, f: &mut dyn io::Write) -> Result<(), io::Error> {
        let ctx = RenderContext::default();

        let mut first = true;

        match &self.file_comment {
            Some(c) => {
                c.render(f, ctx.clone())?;
                first = false;
            }
            None => (),
        }

        if !first && !self.using_statements.is_empty() {
            write!(f, "\n")?;
        }

        for using in &self.using_statements {
            using.render(f, ctx.clone())?;
            first = false;
        }

        for child in &self.children {
            if !first {
                write!(f, "\n")?;
            }

            child.render(f, ctx.clone())?;
            first = false;
        }

        Ok(())
    }
}

pub struct BlockComment {
    pub text: Vec<String>,
}

impl AstNode for BlockComment {
    fn render(&self, f: &mut dyn io::Write, ctx: RenderContext) -> Result<(), io::Error> {
        render_ln!(f, &ctx, "/*")?;
        for line in &self.text {
            render_ln!(f, &ctx, " * {}", line)?;
        }
        render_ln!(f, &ctx, " */")?;

        Ok(())
    }
}

pub struct UsingStatement {
    pub path: String,
}

impl AstNode for UsingStatement {
    fn render(&self, f: &mut dyn io::Write, ctx: RenderContext) -> Result<(), io::Error> {
        render_ln!(f, &ctx, "using {};", self.path)
    }
}

pub struct Namespace {
    pub name: String,
    pub children: Vec<Box<dyn AstNode>>,
}

impl AstNode for Namespace {
    fn render(&self, f: &mut dyn io::Write, ctx: RenderContext) -> Result<(), io::Error> {
        render_ln!(f, &ctx, "namespace {}", self.name)?;
        render_ln!(f, &ctx, "{{")?;

        for child in &self.children {
            child.render(f, ctx.indented())?;
        }

        render_ln!(f, &ctx, "}}")?;

        Ok(())
    }
}

pub struct ImportedMethod {
    pub binary_name: String,
    pub func_data: BindgenFunction,
}

impl ImportedMethod {
    fn csharp_name(&self) -> String {
        self.func_data.name.to_camel_case()
    }
}

impl AstNode for ImportedMethod {
    fn render(&self, f: &mut dyn io::Write, ctx: RenderContext) -> Result<(), io::Error> {
        render_ln!(
            f,
            &ctx,
            "[DllImport(\"{}\", EntryPoint = \"{}\")]",
            self.binary_name,
            self.func_data.name
        )?;

        render_indent(f, &ctx)?;

        write!(f, "public static extern ")?;
        self.func_data.return_type.render(f, ctx.clone())?;
        write!(f, " {}(", self.csharp_name())?;

        // TODO: Implement Iterator for MaybeOwnedArr
        let mut first = true;
        for arg in &self.func_data.args[..] {
            if !first {
                write!(f, ", ")?;
            }

            arg.ffi_type.render(f, ctx.clone())?;
            write!(f, " {}", arg.name.to_mixed_case())?;
            first = false;
        }

        write!(f, ");\n")?;

        Ok(())
    }
}

pub struct Class {
    pub name: String,
    pub methods: Vec<ImportedMethod>,
    pub is_static: bool,
}

impl AstNode for Class {
    fn render(&self, f: &mut dyn io::Write, ctx: RenderContext) -> Result<(), io::Error> {
        let static_part = if self.is_static { "static " } else { "" };
        render_ln!(f, &ctx, "public {}class {}", static_part, self.name)?;
        render_ln!(f, &ctx, "{{")?;

        for method in &self.methods {
            method.render(f, ctx.indented())?;
        }

        render_ln!(f, &ctx, "}}")?;

        Ok(())
    }
}
