//! Experimental native desktop shell: GPUI Kit + `desktop-session`, no WebView / IPC (D-126).
//!
//! ```text
//! cargo run -p gpui-desktop
//! cargo run -p gpui-desktop -- path\to\Library.eglib
//! ```

mod library;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_session::Session;
use gpui_kit::assets::Assets;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{ActiveTheme, Icon, IconName, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, ExternalPaths, Image, ImageFormat, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    StyledImage as _, Window, WindowBounds, WindowOptions, div, img, px, rgb, size,
};
use library_core::ItemCell;

use crate::library::{APP_VERSION, cell_thumbnail, fixture_library_path, open_and_list, session};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let initial_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(fixture_library_path);

    let app = gpui_kit::application().with_assets(Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1100.), px(720.)), cx)),
            titlebar: Some(Default::default()),
            ..Default::default()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| SpikeApp::new(initial_path.clone(), window, cx))
        })
        .expect("failed to open GPUI desktop window");
    });
}

struct ListedItem {
    cell: ItemCell,
    thumb: Option<Arc<Image>>,
}

struct SpikeApp {
    session: Session,
    path_input: Entity<InputState>,
    status: SharedString,
    library_name: Option<SharedString>,
    total: u32,
    items: Vec<ListedItem>,
    read_only: bool,
    /// Keeps a temp copy of the repo fixture alive for the process lifetime.
    fixture_scratch: Option<PathBuf>,
    /// Last OS file-drop paths (Phase 0.6 probe); first image may feed [`drop_preview`].
    drop_paths: Vec<PathBuf>,
    drop_preview: Option<Arc<Image>>,
}

impl SpikeApp {
    fn new(initial_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(initial_path.to_string_lossy().as_ref())
        });

        let mut app = Self {
            session: session(),
            path_input,
            status: SharedString::from(format!(
                "GPUI Kit spike · desktop-session {APP_VERSION} · pick a .eglib and Open (drop files onto the window to probe OS DnD)"
            )),
            library_name: None,
            total: 0,
            items: Vec::new(),
            read_only: false,
            fixture_scratch: None,
            drop_paths: Vec::new(),
            drop_preview: None,
        };

        // Auto-open argv / the repo fixture when the folder exists. The checked-in
        // fixture is copied so migration can run without mutating git files.
        if is_repo_fixture(&initial_path) {
            app.open_fixture(window, cx);
        } else if initial_path.is_dir() {
            app.open_current_path(window, cx);
        }
        app
    }

    fn open_fixture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match copy_fixture_library() {
            Ok(path) => {
                self.fixture_scratch = Some(path.clone());
                self.path_input.update(cx, |input, cx| {
                    input.set_value(path.to_string_lossy().as_ref(), window, cx);
                });
                self.open_path(path, cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("Fixture copy failed: {error}"));
                cx.notify();
            }
        }
    }

    fn open_current_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = PathBuf::from(self.path_input.read(cx).value().to_string());
        if is_repo_fixture(&path) {
            self.open_fixture(window, cx);
            return;
        }
        self.open_path(path, cx);
    }

    fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match self.try_open(&path) {
            Ok(()) => cx.notify(),
            Err(error) => {
                self.status = SharedString::from(format!("Open failed: {error}"));
                self.library_name = None;
                self.total = 0;
                self.items.clear();
                cx.notify();
            }
        }
    }

    /// Phase 0.6: OS file drop → status + optional `img` of the first image path.
    /// Does not import into the library (that is Phase 3).
    fn on_os_file_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        let list: Vec<PathBuf> = paths.paths().to_vec();
        let summary = if list.is_empty() {
            "OS drop: empty path list".to_string()
        } else {
            let names: Vec<String> = list
                .iter()
                .take(5)
                .map(|p| p.display().to_string())
                .collect();
            let more = if list.len() > 5 {
                format!(" (+{} more)", list.len() - 5)
            } else {
                String::new()
            };
            format!("OS drop OK ({} path{}): {}{}", list.len(), if list.len() == 1 { "" } else { "s" }, names.join(", "), more)
        };
        self.drop_preview = list.iter().find_map(|path| load_drop_preview(path));
        self.drop_paths = list;
        self.status = SharedString::from(summary);
        cx.notify();
    }

    fn try_open(&mut self, path: &std::path::Path) -> Result<(), library_core::SourceError> {
        let (info, opened, cells) = open_and_list(&self.session, path, 0, 120)?;
        self.library_name = Some(SharedString::from(info.name.clone()));
        self.total = opened.total;
        self.read_only = info.read_only;
        self.items = cells
            .into_iter()
            .map(|cell| {
                let thumb = cell_thumbnail(&self.session, &cell).and_then(|thumb| {
                    let format = match thumb.content_type.as_str() {
                        "image/webp" => ImageFormat::Webp,
                        "image/png" => ImageFormat::Png,
                        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
                        "image/gif" => ImageFormat::Gif,
                        _ => return None,
                    };
                    Some(Arc::new(Image::from_bytes(format, thumb.bytes)))
                });
                ListedItem { cell, thumb }
            })
            .collect();
        let mode = if self.read_only {
            "read-only"
        } else {
            "writable"
        };
        self.status = SharedString::from(format!(
            "Opened {} ({mode}) · {} items · view #{}",
            path.display(),
            self.total,
            opened.view_id
        ));
        Ok(())
    }
}

impl Render for SpikeApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .p_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Icon::new(IconName::GalleryVerticalEnd))
                                    .child(
                                        Label::new("egl-viewer · GPUI desktop (experiment)")
                                            .font_bold(),
                                    ),
                            )
                            .child(
                                Label::new(format!("gpui-kit 0.9.0 · {APP_VERSION}"))
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().child(Input::new(&self.path_input)))
                            .child(Button::new("open").primary().label("Open").on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_current_path(window, cx);
                                }),
                            ))
                            .child(Button::new("fixture").label("Open fixture").on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_fixture(window, cx);
                                }),
                            )),
                    )
                    .child(
                        Label::new(self.status.clone())
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .when_some(self.library_name.clone(), |el, name| {
                        el.child(
                            Label::new(format!(
                                "Library: {name} · showing {} of {}",
                                self.items.len(),
                                self.total
                            ))
                            .text_sm(),
                        )
                    })
                    .when(!self.drop_paths.is_empty(), |el| {
                        let preview = self.drop_preview.clone();
                        el.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Label::new(format!(
                                        "Last drop: {} path{}",
                                        self.drop_paths.len(),
                                        if self.drop_paths.len() == 1 { "" } else { "s" }
                                    ))
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                                )
                                .when_some(preview, |row, image| {
                                    row.child(
                                        img(image)
                                            .w(px(64.))
                                            .h(px(64.))
                                            .object_fit(gpui_kit::ObjectFit::Contain),
                                    )
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .id("item-grid")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .p_4()
                    // OS file drop lands as ExternalPaths on active_drag; on_drop receives it.
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                        this.on_os_file_drop(paths, cx);
                    }))
                    .child(if self.items.is_empty() {
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Label::new(
                                    "No items yet. Open a native .eglib (or the repo fixture). Drop files here to probe OS DnD.",
                                )
                                .text_color(theme.muted_foreground),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .children(self.items.iter().enumerate().map(|(index, item)| {
                                item_card(
                                    index,
                                    item,
                                    theme.muted,
                                    theme.muted_foreground,
                                    theme.border,
                                    theme.radius,
                                    theme.secondary,
                                )
                            }))
                            .into_any_element()
                    }),
            )
    }
}

/// Best-effort decode of a dropped image for the Phase 0.6 `img` probe.
fn load_drop_preview(path: &Path) -> Option<Arc<Image>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = match ext.as_str() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

fn item_card(
    index: usize,
    item: &ListedItem,
    muted: gpui_kit::Hsla,
    muted_foreground: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    radius: gpui_kit::Pixels,
    secondary: gpui_kit::Hsla,
) -> impl IntoElement {
    let cell = &item.cell;
    let title = format!("{}.{}", cell.name, cell.ext);
    let meta = match (cell.width, cell.height) {
        (Some(w), Some(h)) => format!("{w}×{h}"),
        _ => cell.ext.clone(),
    };

    let preview = match &item.thumb {
        Some(image) => img(image.clone())
            .w(px(140.))
            .h(px(140.))
            .object_fit(gpui_kit::ObjectFit::Cover)
            .into_any_element(),
        None => {
            let color = cell.palette0.map_or(muted, |[r, g, b]| {
                rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)).into()
            });
            div()
                .w(px(140.))
                .h(px(140.))
                .bg(color)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Label::new(cell.ext.to_uppercase())
                        .text_xs()
                        .text_color(muted_foreground),
                )
                .into_any_element()
        }
    };

    v_flex()
        .id(SharedString::from(format!("item-{index}")))
        .w(px(148.))
        .gap_1()
        .rounded(radius)
        .border_1()
        .border_color(border)
        .p_1()
        .bg(secondary)
        .child(preview)
        .child(
            Label::new(title)
                .text_sm()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis(),
        )
        .child(Label::new(meta).text_xs().text_color(muted_foreground))
}

fn is_repo_fixture(path: &Path) -> bool {
    let Ok(wanted) = fixture_library_path().canonicalize() else {
        return false;
    };
    path.canonicalize().is_ok_and(|got| got == wanted)
}

/// Copies `fixtures/library/v5/Fixture.eglib` into a fresh temp folder so open can migrate.
fn copy_fixture_library() -> std::io::Result<PathBuf> {
    let source = fixture_library_path();
    let scratch = std::env::temp_dir().join(format!(
        "egl-gpui-fixture-{}-{}",
        std::process::id(),
        library::now_stamp()
    ));
    let root = scratch.join("Fixture.eglib");
    copy_dir(&source, &root)?;
    Ok(root)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
