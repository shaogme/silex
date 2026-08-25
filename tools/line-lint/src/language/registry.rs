use std::path::Path;

use super::{
    LanguageAnalyzer, LanguageError, c::CAnalyzer, cmake::CmakeAnalyzer, cpp::CppAnalyzer,
    csharp::CSharpAnalyzer, css::CssAnalyzer, dart::DartAnalyzer, dockerfile::DockerfileAnalyzer,
    elixir::ElixirAnalyzer, fish::FishAnalyzer, go::GoAnalyzer, groovy::GroovyAnalyzer,
    haskell::HaskellAnalyzer, html::HtmlAnalyzer, ini::IniAnalyzer, java::JavaAnalyzer,
    javascript::JavaScriptAnalyzer, julia::JuliaAnalyzer, kotlin::KotlinAnalyzer,
    less::LessAnalyzer, makefile::MakefileAnalyzer, markdown::MarkdownAnalyzer, perl::PerlAnalyzer,
    php::PhpAnalyzer, python::PythonAnalyzer, r::RAnalyzer, ruby::RubyAnalyzer, rust::RustAnalyzer,
    scala::ScalaAnalyzer, scss::ScssAnalyzer, shell::ShellAnalyzer, sql::SqlAnalyzer,
    svg::SvgAnalyzer, swift::SwiftAnalyzer, toml::TomlAnalyzer, typescript::TypeScriptAnalyzer,
    vue::VueAnalyzer, xml::XmlAnalyzer, yaml::YamlAnalyzer, zig::ZigAnalyzer,
};

static C: CAnalyzer = CAnalyzer;
static CPP: CppAnalyzer = CppAnalyzer;
static CSHARP: CSharpAnalyzer = CSharpAnalyzer;
static DART: DartAnalyzer = DartAnalyzer;
static GO: GoAnalyzer = GoAnalyzer;
static GROOVY: GroovyAnalyzer = GroovyAnalyzer;
static JAVA: JavaAnalyzer = JavaAnalyzer;
static JAVASCRIPT: JavaScriptAnalyzer = JavaScriptAnalyzer;
static KOTLIN: KotlinAnalyzer = KotlinAnalyzer;
static PHP: PhpAnalyzer = PhpAnalyzer;
static SCALA: ScalaAnalyzer = ScalaAnalyzer;
static SWIFT: SwiftAnalyzer = SwiftAnalyzer;
static SVG: SvgAnalyzer = SvgAnalyzer;
static TYPESCRIPT: TypeScriptAnalyzer = TypeScriptAnalyzer;
static ZIG: ZigAnalyzer = ZigAnalyzer;
static ELIXIR: ElixirAnalyzer = ElixirAnalyzer;
static FISH: FishAnalyzer = FishAnalyzer;
static INI: IniAnalyzer = IniAnalyzer;
static JULIA: JuliaAnalyzer = JuliaAnalyzer;
static PERL: PerlAnalyzer = PerlAnalyzer;
static R: RAnalyzer = RAnalyzer;
static RUBY: RubyAnalyzer = RubyAnalyzer;
static SHELL: ShellAnalyzer = ShellAnalyzer;
static TOML: TomlAnalyzer = TomlAnalyzer;
static YAML: YamlAnalyzer = YamlAnalyzer;
static SQL: SqlAnalyzer = SqlAnalyzer;
static HASKELL: HaskellAnalyzer = HaskellAnalyzer;
static CSS: CssAnalyzer = CssAnalyzer;
static LESS: LessAnalyzer = LessAnalyzer;
static SCSS: ScssAnalyzer = ScssAnalyzer;
static VUE: VueAnalyzer = VueAnalyzer;
static HTML: HtmlAnalyzer = HtmlAnalyzer;
static XML: XmlAnalyzer = XmlAnalyzer;
static MARKDOWN: MarkdownAnalyzer = MarkdownAnalyzer;
static DOCKERFILE: DockerfileAnalyzer = DockerfileAnalyzer;
static MAKEFILE: MakefileAnalyzer = MakefileAnalyzer;
static CMAKE: CmakeAnalyzer = CmakeAnalyzer;
static RUST: RustAnalyzer = RustAnalyzer;
static PYTHON: PythonAnalyzer = PythonAnalyzer;

static ANALYZERS: &[&dyn LanguageAnalyzer] = &[
    &RUST,
    &PYTHON,
    &C,
    &CPP,
    &CSHARP,
    &DART,
    &GO,
    &GROOVY,
    &JAVA,
    &JAVASCRIPT,
    &KOTLIN,
    &PHP,
    &SCALA,
    &SWIFT,
    &SVG,
    &TYPESCRIPT,
    &ZIG,
    &ELIXIR,
    &FISH,
    &INI,
    &JULIA,
    &PERL,
    &R,
    &RUBY,
    &SHELL,
    &TOML,
    &YAML,
    &SQL,
    &HASKELL,
    &CSS,
    &LESS,
    &SCSS,
    &VUE,
    &HTML,
    &XML,
    &MARKDOWN,
    &DOCKERFILE,
    &MAKEFILE,
    &CMAKE,
];

struct IndexEntry {
    value: &'static str,
    analyzer: &'static dyn LanguageAnalyzer,
}

static FILE_NAME_INDEX: &[IndexEntry] = &[
    IndexEntry {
        value: "Dockerfile",
        analyzer: &DOCKERFILE,
    },
    IndexEntry {
        value: "Makefile",
        analyzer: &MAKEFILE,
    },
    IndexEntry {
        value: "CMakeLists.txt",
        analyzer: &CMAKE,
    },
];

static EXTENSION_INDEX: &[IndexEntry] = &[
    IndexEntry {
        value: "rs",
        analyzer: &RUST,
    },
    IndexEntry {
        value: "py",
        analyzer: &PYTHON,
    },
    IndexEntry {
        value: "c",
        analyzer: &C,
    },
    IndexEntry {
        value: "cc",
        analyzer: &CPP,
    },
    IndexEntry {
        value: "cpp",
        analyzer: &CPP,
    },
    IndexEntry {
        value: "cxx",
        analyzer: &CPP,
    },
    IndexEntry {
        value: "cs",
        analyzer: &CSHARP,
    },
    IndexEntry {
        value: "dart",
        analyzer: &DART,
    },
    IndexEntry {
        value: "go",
        analyzer: &GO,
    },
    IndexEntry {
        value: "groovy",
        analyzer: &GROOVY,
    },
    IndexEntry {
        value: "java",
        analyzer: &JAVA,
    },
    IndexEntry {
        value: "js",
        analyzer: &JAVASCRIPT,
    },
    IndexEntry {
        value: "jsx",
        analyzer: &JAVASCRIPT,
    },
    IndexEntry {
        value: "kt",
        analyzer: &KOTLIN,
    },
    IndexEntry {
        value: "kts",
        analyzer: &KOTLIN,
    },
    IndexEntry {
        value: "php",
        analyzer: &PHP,
    },
    IndexEntry {
        value: "scala",
        analyzer: &SCALA,
    },
    IndexEntry {
        value: "swift",
        analyzer: &SWIFT,
    },
    IndexEntry {
        value: "ts",
        analyzer: &TYPESCRIPT,
    },
    IndexEntry {
        value: "tsx",
        analyzer: &TYPESCRIPT,
    },
    IndexEntry {
        value: "zig",
        analyzer: &ZIG,
    },
    IndexEntry {
        value: "ex",
        analyzer: &ELIXIR,
    },
    IndexEntry {
        value: "exs",
        analyzer: &ELIXIR,
    },
    IndexEntry {
        value: "fish",
        analyzer: &FISH,
    },
    IndexEntry {
        value: "ini",
        analyzer: &INI,
    },
    IndexEntry {
        value: "jl",
        analyzer: &JULIA,
    },
    IndexEntry {
        value: "pl",
        analyzer: &PERL,
    },
    IndexEntry {
        value: "pm",
        analyzer: &PERL,
    },
    IndexEntry {
        value: "r",
        analyzer: &R,
    },
    IndexEntry {
        value: "rb",
        analyzer: &RUBY,
    },
    IndexEntry {
        value: "sh",
        analyzer: &SHELL,
    },
    IndexEntry {
        value: "toml",
        analyzer: &TOML,
    },
    IndexEntry {
        value: "yaml",
        analyzer: &YAML,
    },
    IndexEntry {
        value: "yml",
        analyzer: &YAML,
    },
    IndexEntry {
        value: "sql",
        analyzer: &SQL,
    },
    IndexEntry {
        value: "hs",
        analyzer: &HASKELL,
    },
    IndexEntry {
        value: "lhs",
        analyzer: &HASKELL,
    },
    IndexEntry {
        value: "css",
        analyzer: &CSS,
    },
    IndexEntry {
        value: "less",
        analyzer: &LESS,
    },
    IndexEntry {
        value: "scss",
        analyzer: &SCSS,
    },
    IndexEntry {
        value: "vue",
        analyzer: &VUE,
    },
    IndexEntry {
        value: "html",
        analyzer: &HTML,
    },
    IndexEntry {
        value: "htm",
        analyzer: &HTML,
    },
    IndexEntry {
        value: "xml",
        analyzer: &XML,
    },
    IndexEntry {
        value: "svg",
        analyzer: &SVG,
    },
    IndexEntry {
        value: "md",
        analyzer: &MARKDOWN,
    },
    IndexEntry {
        value: "markdown",
        analyzer: &MARKDOWN,
    },
];

#[derive(Clone, Copy)]
pub struct LanguageRegistry {
    analyzers: &'static [&'static dyn LanguageAnalyzer],
    file_names: &'static [IndexEntry],
    extensions: &'static [IndexEntry],
}

impl LanguageRegistry {
    pub const fn new() -> Self {
        Self {
            analyzers: ANALYZERS,
            file_names: FILE_NAME_INDEX,
            extensions: EXTENSION_INDEX,
        }
    }

    pub fn analyzer_for(
        &self,
        path: &Path,
    ) -> Result<&'static dyn LanguageAnalyzer, LanguageError> {
        let file_name = path.file_name().and_then(|name| name.to_str());
        if let Some(file_name) = file_name
            && let Some(entry) = self
                .file_names
                .iter()
                .find(|entry| entry.value.eq_ignore_ascii_case(file_name))
        {
            return Ok(entry.analyzer);
        }

        let extension = path.extension().and_then(|extension| extension.to_str());
        if let Some(extension) = extension
            && let Some(entry) = self
                .extensions
                .iter()
                .find(|entry| entry.value.eq_ignore_ascii_case(extension))
        {
            return Ok(entry.analyzer);
        }

        Err(LanguageError::UnsupportedLanguage(path.to_path_buf()))
    }

    pub fn analyzers(&self) -> &'static [&'static dyn LanguageAnalyzer] {
        self.analyzers
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
