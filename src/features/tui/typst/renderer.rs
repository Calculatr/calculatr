use std::sync::OnceLock;

use image::RgbaImage;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

struct Shared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        let mut book = FontBook::new();
        for font in &fonts {
            book.push(font.info().clone());
        }
        Shared {
            library: LazyHash::new(Library::builder().build()),
            book: LazyHash::new(book),
            fonts,
        }
    })
}

pub struct TypstRenderer {
    source: Source,
}

impl TypstRenderer {
    pub fn new(content: String) -> Self {
        let vpath = VirtualPath::new("main.typ").expect("static path is valid");
        let file_id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
        Self {
            source: Source::new(file_id, content),
        }
    }

    pub fn render(&self, pixel_per_pt: f32) -> Result<RgbaImage, String> {
        let warned = typst::compile::<PagedDocument>(self);
        let document = warned
            .output
            .map_err(|e| format!("Typst compilation error: {e:?}"))?;

        let page = document
            .pages()
            .first()
            .ok_or_else(|| "Typst generated zero pages".to_string())?;

        let options = typst_render::RenderOptions {
            pixel_per_pt: f64::from(pixel_per_pt).into(),
            ..Default::default()
        };
        let png = typst_render::render(page, &options)
            .encode_png()
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        image::load_from_memory(&png)
            .map(|img| img.to_rgba8())
            .map_err(|e| format!("PNG decoding failed: {e}"))
    }
}

impl World for TypstRenderer {
    fn library(&self) -> &LazyHash<Library> {
        &shared().library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &shared().book
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
        shared().fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
