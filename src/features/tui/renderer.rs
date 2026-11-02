use std::sync::OnceLock;

use ratatui::style::Color;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

use crate::features::tui::theme::Theme;

/// Eingebettete Typst-Fonts (inkl. New Computer Modern Math), einmal pro Prozess geladen.
static FONTS: OnceLock<(FontBook, Vec<Font>)> = OnceLock::new();

fn embedded_fonts() -> &'static (FontBook, Vec<Font>) {
    FONTS.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        let mut book = FontBook::new();
        for font in &fonts {
            book.push(font.info().clone());
        }
        (book, fonts)
    })
}

pub struct TypstRenderer {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    source: Source,
}

impl TypstRenderer {
    pub fn new(math_code: &str, theme: &Theme) -> Self {
        let color_hex = match theme.input_text {
            Color::Rgb(r, g, b) => format!("rgb(\"{:02x}{:02x}{:02x}\")", r, g, b),
            Color::Cyan => "rgb(\"00ffff\")".to_string(),
            Color::Magenta => "rgb(\"ff00ff\")".to_string(),
            Color::Green => "rgb(\"00ff00\")".to_string(),
            Color::Yellow => "rgb(\"ffff00\")".to_string(),
            _ => "white".to_string(),
        };

        let content = format!(
            "#set page(width: auto, height: auto, fill: none, margin: (x: 4pt, y: 4pt))\n#set text(fill: {})\n$ {} $",
            color_hex, math_code
        );

        let library = LazyHash::new(Library::builder().build());
        let (book, fonts) = embedded_fonts();

        // Typst 0.15: FileId ist ein interned RootedPath (Root + VirtualPath)
        let vpath = VirtualPath::new("main.typ").expect("static path is valid");
        let file_id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
        let source = Source::new(file_id, content);

        Self {
            library,
            book: LazyHash::new(book.clone()),
            fonts: fonts.clone(),
            source,
        }
    }

    pub fn render_to_png(&self) -> Result<Vec<u8>, String> {
        let warned = typst::compile::<PagedDocument>(self);
        let document = warned
            .output
            .map_err(|e| format!("Typst compilation error: {:?}", e))?;

        let page = document
            .pages()
            .first()
            .ok_or_else(|| "Typst generated zero pages".to_string())?;

        let options = typst_render::RenderOptions {
            pixel_per_pt: 2.0.into(), // 2x für HiDPI
            ..Default::default()
        };

        let pixmap = typst_render::render(page, &options);
        pixmap
            .encode_png()
            .map_err(|e| format!("PNG encoding failed: {}", e))
    }
}

impl World for TypstRenderer {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_with_slash().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().get_with_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
