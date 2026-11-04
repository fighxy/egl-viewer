import EglCore
import Foundation
import Observation

/// One item's details and its edits (D-095): name, rating, tags, folders, notes and link.
/// Every edit goes through the core as one undoable step and the model takes the item it
/// returns.
@MainActor
@Observable
final class ItemDetailsModel {
    let open: OpenLibrary
    let id: String
    private(set) var item: Item?
    private(set) var vocabulary: [Tag] = []

    init(open: OpenLibrary, id: String) {
        self.open = open
        self.id = id
    }

    var readOnly: Bool { open.readOnly }

    func load() async {
        let id = id
        item = await open.perform { try $0.item(id) }
        if let tags = await open.perform({ try $0.tags() }) {
            vocabulary = tags.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        }
    }

    func rename(_ name: String) async {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name != item?.name else { return }
        await apply(.rename(name))
    }

    /// Tapping the current rating again clears it.
    func rate(_ stars: Int) async {
        await apply(.setStar(item?.star == stars ? 0 : stars))
    }

    func addTag(_ name: String) async {
        guard let item, !item.tags.contains(name) else { return }
        await apply(.addTags([name]))
    }

    func removeTag(_ name: String) async {
        await apply(.removeTags([name]))
    }

    /// In the folder: out of it; otherwise into it.
    func toggleFolder(_ folder: String) async {
        guard let item else { return }
        await apply(item.folders.contains(folder) ? .removeFromFolders([folder]) : .addToFolders([folder]))
    }

    func setNotes(_ text: String) async {
        guard let item, text != item.annotation else { return }
        await apply(.setAnnotation(text))
    }

    func setLink(_ text: String) async {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let item, text != item.url else { return }
        await apply(.setUrl(text))
    }

    /// Folder names of the item, in tree order; folders the app cannot see are left out.
    var folderNames: [(id: String, name: String)] {
        guard let item else { return [] }
        return open.pickableFolders
            .filter { item.folders.contains($0.folder.id) }
            .map { (id: $0.folder.id, name: $0.folder.name) }
    }

    private func apply(_ edit: ItemEdit) async {
        guard !readOnly else { return }
        let id = id
        if let updated = await open.perform({ try $0.edit(id, edit) }) {
            item = updated
        }
    }
}
