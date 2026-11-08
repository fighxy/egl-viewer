//! Minimal string catalogs for Welcome / shell chrome (Phase 1). Ports keys from `ui/src/i18n`.

use app_store::Locale;
use std::collections::HashMap;
use std::sync::OnceLock;

type Catalog = HashMap<&'static str, &'static str>;

fn en() -> Catalog {
    HashMap::from([
        ("common.cancel", "Cancel"),
        ("common.save", "Save"),
        ("common.browse", "Browse…"),
        ("common.missing", "Missing"),
        ("common.readOnly", "Read-only"),
        ("menus.file", "File"),
        ("menus.edit", "Edit"),
        ("menus.view", "View"),
        ("menus.library", "Library"),
        ("menus.help", "Help"),
        ("commands.file.newLibrary", "New Library…"),
        ("commands.file.openLibrary", "Open Library…"),
        ("commands.file.openRecent", "Open Recent"),
        ("commands.file.closeLibrary", "Close Library"),
        ("commands.file.settings", "Settings…"),
        ("commands.file.exit", "Exit"),
        ("commands.edit.selectAll", "Select All"),
        ("commands.edit.find", "Find"),
        ("commands.view.theme", "Theme"),
        ("commands.view.fullScreen", "Full Screen"),
        ("commands.view.sidebar", "Sidebar"),
        ("commands.view.details", "Details Panel"),
        ("commands.view.grid", "Grid"),
        ("commands.view.list", "List"),
        ("commands.view.larger", "Larger Thumbnails"),
        ("commands.view.smaller", "Smaller Thumbnails"),
        ("commands.view.showNames", "Show Names"),
        ("commands.view.sortBy", "Sort By"),
        ("commands.view.preview", "Preview"),
        ("commands.view.includeSubfolders", "Include Subfolders"),
        ("commands.help.shortcuts", "Keyboard Shortcuts"),
        ("commands.help.about", "About egl-viewer"),
        ("commands.help.openLogFolder", "Open Log Folder"),
        ("themes.system", "System"),
        ("themes.light", "Light"),
        ("themes.dark", "Dark"),
        ("welcome.title", "Welcome to egl-viewer"),
        ("welcome.description", "Keep your images and references in a library on this PC."),
        ("welcome.create", "Create library"),
        ("welcome.open", "Open library"),
        ("welcome.recent", "Recent libraries"),
        ("welcome.noRecent", "No recent libraries yet."),
        ("welcome.removeRecentTitle", "Remove from the list"),
        ("welcome.eagle", "Eagle"),
        ("welcome.importEagle", "Import an Eagle library"),
        ("welcome.connectEagle", "Connect to a running Eagle"),
        ("welcome.eagleLater", "Eagle import and live connect arrive in a later phase."),
        ("createLibrary.title", "Create library"),
        ("createLibrary.name", "Name"),
        ("createLibrary.location", "Location"),
        ("createLibrary.create", "Create"),
        ("shell.all", "All"),
        ("shell.uncategorized", "Uncategorized"),
        ("shell.untagged", "Untagged"),
        ("shell.trash", "Trash"),
        ("shell.folders", "Folders"),
        ("shell.search", "Search"),
        ("shell.statusItems", "{n} items"),
        ("shell.statusSelected", "{n} selected"),
        ("shell.readOnlyBanner", "This library is open read-only."),
        ("shell.detailsStub", "Details (read-only stub)"),
        ("shell.noItems", "No items in this view."),
        ("settings.title", "Settings"),
        ("settings.general", "General"),
        ("settings.theme", "Theme"),
        ("settings.language", "Language"),
        ("settings.reopen", "Reopen the last library on start"),
        ("settings.confirmTrash", "Ask before moving items to the trash"),
        ("settings.defaultFolder", "Default library folder"),
        ("about.title", "About egl-viewer"),
        ("about.version", "Version"),
        ("about.toolkit", "UI toolkit"),
        ("about.os", "Operating system"),
        ("shortcuts.title", "Keyboard Shortcuts"),
        ("quit.title", "Quit egl-viewer?"),
        ("quit.message", "Close the app? Open imports or exports (when added) will stop."),
        ("quit.confirm", "Quit"),
    ])
}

fn ru() -> Catalog {
    HashMap::from([
        ("common.cancel", "Отмена"),
        ("common.save", "Сохранить"),
        ("common.browse", "Обзор…"),
        ("common.missing", "Нет"),
        ("common.readOnly", "Только чтение"),
        ("menus.file", "Файл"),
        ("menus.edit", "Правка"),
        ("menus.view", "Вид"),
        ("menus.library", "Библиотека"),
        ("menus.help", "Справка"),
        ("commands.file.newLibrary", "Новая библиотека…"),
        ("commands.file.openLibrary", "Открыть библиотеку…"),
        ("commands.file.openRecent", "Недавние"),
        ("commands.file.closeLibrary", "Закрыть библиотеку"),
        ("commands.file.settings", "Параметры…"),
        ("commands.file.exit", "Выход"),
        ("commands.edit.selectAll", "Выделить всё"),
        ("commands.edit.find", "Найти"),
        ("commands.view.theme", "Тема"),
        ("commands.view.fullScreen", "Полный экран"),
        ("commands.view.sidebar", "Боковая панель"),
        ("commands.view.details", "Панель сведений"),
        ("commands.view.grid", "Сетка"),
        ("commands.view.list", "Список"),
        ("commands.view.larger", "Крупнее"),
        ("commands.view.smaller", "Мельче"),
        ("commands.view.showNames", "Показывать имена"),
        ("commands.view.sortBy", "Сортировка"),
        ("commands.view.preview", "Просмотр"),
        ("commands.view.includeSubfolders", "Включая вложенные"),
        ("commands.help.shortcuts", "Сочетания клавиш"),
        ("commands.help.about", "О программе"),
        ("commands.help.openLogFolder", "Открыть папку журналов"),
        ("themes.system", "Системная"),
        ("themes.light", "Светлая"),
        ("themes.dark", "Тёмная"),
        ("welcome.title", "Добро пожаловать в egl-viewer"),
        ("welcome.description", "Храните изображения и референсы в библиотеке на этом ПК."),
        ("welcome.create", "Создать библиотеку"),
        ("welcome.open", "Открыть библиотеку"),
        ("welcome.recent", "Недавние библиотеки"),
        ("welcome.noRecent", "Пока нет недавних библиотек."),
        ("welcome.removeRecentTitle", "Убрать из списка"),
        ("welcome.eagle", "Eagle"),
        ("welcome.importEagle", "Импорт библиотеки Eagle"),
        ("welcome.connectEagle", "Подключиться к запущенному Eagle"),
        ("welcome.eagleLater", "Импорт Eagle и живое подключение — в следующей фазе."),
        ("createLibrary.title", "Создать библиотеку"),
        ("createLibrary.name", "Имя"),
        ("createLibrary.location", "Расположение"),
        ("createLibrary.create", "Создать"),
        ("shell.all", "Все"),
        ("shell.uncategorized", "Без папки"),
        ("shell.untagged", "Без тегов"),
        ("shell.trash", "Корзина"),
        ("shell.folders", "Папки"),
        ("shell.search", "Поиск"),
        ("shell.statusItems", "{n} объектов"),
        ("shell.statusSelected", "{n} выбрано"),
        ("shell.readOnlyBanner", "Библиотека открыта только для чтения."),
        ("shell.detailsStub", "Сведения (заглушка)"),
        ("shell.noItems", "В этом виде нет объектов."),
        ("settings.title", "Параметры"),
        ("settings.general", "Общие"),
        ("settings.theme", "Тема"),
        ("settings.language", "Язык"),
        ("settings.reopen", "Открывать последнюю библиотеку при запуске"),
        ("settings.confirmTrash", "Спрашивать перед перемещением в корзину"),
        ("settings.defaultFolder", "Папка для новых библиотек"),
        ("about.title", "О программе egl-viewer"),
        ("about.version", "Версия"),
        ("about.toolkit", "Интерфейс"),
        ("about.os", "ОС"),
        ("shortcuts.title", "Сочетания клавиш"),
        ("quit.title", "Выйти из egl-viewer?"),
        ("quit.message", "Закрыть приложение?"),
        ("quit.confirm", "Выйти"),
    ])
}

fn zh_cn() -> Catalog {
    HashMap::from([
        ("common.cancel", "取消"),
        ("common.save", "保存"),
        ("common.browse", "浏览…"),
        ("common.missing", "缺失"),
        ("common.readOnly", "只读"),
        ("menus.file", "文件"),
        ("menus.edit", "编辑"),
        ("menus.view", "查看"),
        ("menus.library", "图库"),
        ("menus.help", "帮助"),
        ("commands.file.newLibrary", "新建图库…"),
        ("commands.file.openLibrary", "打开图库…"),
        ("commands.file.openRecent", "最近打开"),
        ("commands.file.closeLibrary", "关闭图库"),
        ("commands.file.settings", "设置…"),
        ("commands.file.exit", "退出"),
        ("commands.edit.selectAll", "全选"),
        ("commands.edit.find", "查找"),
        ("commands.view.theme", "主题"),
        ("commands.view.fullScreen", "全屏"),
        ("commands.view.sidebar", "侧栏"),
        ("commands.view.details", "详细信息"),
        ("commands.view.grid", "网格"),
        ("commands.view.list", "列表"),
        ("commands.view.larger", "放大缩略图"),
        ("commands.view.smaller", "缩小缩略图"),
        ("commands.view.showNames", "显示名称"),
        ("commands.view.sortBy", "排序"),
        ("commands.view.preview", "预览"),
        ("commands.view.includeSubfolders", "包含子文件夹"),
        ("commands.help.shortcuts", "键盘快捷键"),
        ("commands.help.about", "关于 egl-viewer"),
        ("commands.help.openLogFolder", "打开日志文件夹"),
        ("themes.system", "跟随系统"),
        ("themes.light", "浅色"),
        ("themes.dark", "深色"),
        ("welcome.title", "欢迎使用 egl-viewer"),
        ("welcome.description", "在本机图库中保存图片与设计参考。"),
        ("welcome.create", "创建图库"),
        ("welcome.open", "打开图库"),
        ("welcome.recent", "最近的图库"),
        ("welcome.noRecent", "还没有最近的图库。"),
        ("welcome.removeRecentTitle", "从列表中移除"),
        ("welcome.eagle", "Eagle"),
        ("welcome.importEagle", "导入 Eagle 图库"),
        ("welcome.connectEagle", "连接到正在运行的 Eagle"),
        ("welcome.eagleLater", "Eagle 导入与实时连接将在后续阶段提供。"),
        ("createLibrary.title", "创建图库"),
        ("createLibrary.name", "名称"),
        ("createLibrary.location", "位置"),
        ("createLibrary.create", "创建"),
        ("shell.all", "全部"),
        ("shell.uncategorized", "未分类"),
        ("shell.untagged", "无标签"),
        ("shell.trash", "回收站"),
        ("shell.folders", "文件夹"),
        ("shell.search", "搜索"),
        ("shell.statusItems", "{n} 项"),
        ("shell.statusSelected", "已选 {n}"),
        ("shell.readOnlyBanner", "此图库为只读打开。"),
        ("shell.detailsStub", "详细信息（只读占位）"),
        ("shell.noItems", "此视图中没有项目。"),
        ("settings.title", "设置"),
        ("settings.general", "常规"),
        ("settings.theme", "主题"),
        ("settings.language", "语言"),
        ("settings.reopen", "启动时重新打开上次图库"),
        ("settings.confirmTrash", "移入回收站前询问"),
        ("settings.defaultFolder", "默认图库文件夹"),
        ("about.title", "关于 egl-viewer"),
        ("about.version", "版本"),
        ("about.toolkit", "界面工具包"),
        ("about.os", "操作系统"),
        ("shortcuts.title", "键盘快捷键"),
        ("quit.title", "退出 egl-viewer？"),
        ("quit.message", "关闭应用？"),
        ("quit.confirm", "退出"),
    ])
}

fn catalog(locale: Locale) -> &'static Catalog {
    static EN: OnceLock<Catalog> = OnceLock::new();
    static RU: OnceLock<Catalog> = OnceLock::new();
    static ZH: OnceLock<Catalog> = OnceLock::new();
    match locale {
        Locale::En => EN.get_or_init(en),
        Locale::Ru => RU.get_or_init(ru),
        Locale::ZhCn => ZH.get_or_init(zh_cn),
    }
}

/// Looks up `key` in the active locale, falling back to English, then the key itself.
pub fn t(locale: Locale, key: &str) -> String {
    catalog(locale)
        .get(key)
        .or_else(|| catalog(Locale::En).get(key))
        .copied()
        .unwrap_or(key)
        .to_string()
}

/// Replaces `{n}` in a translated template.
pub fn t_n(locale: Locale, key: &str, n: u32) -> String {
    t(locale, key).replace("{n}", &n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_welcome_title() {
        assert_eq!(t(Locale::En, "welcome.title"), "Welcome to egl-viewer");
    }

    #[test]
    fn russian_and_chinese_have_welcome() {
        assert!(t(Locale::Ru, "welcome.create").contains("Создать"));
        assert!(t(Locale::ZhCn, "welcome.open").contains("打开"));
    }

    #[test]
    fn missing_key_falls_back_to_key() {
        assert_eq!(t(Locale::En, "no.such.key"), "no.such.key");
    }
}
