import EglCore
import SwiftUI
import UIKit

/// Settings (D-099): the open library's settings and upkeep, then the app's own (the login,
/// the version). From the Welcome screen only the app's part shows.
struct SettingsScreen: View {
    let open: OpenLibrary?
    @Environment(AppLock.self) private var lock
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if let open {
                    Section {
                        NavigationLink {
                            LibraryInfoView(open: open)
                        } label: {
                            Label {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(verbatim: open.info.name)
                                    Text("settings.libraryDetail")
                                        .font(.caption)
                                        .foregroundStyle(EglTokens.mutedForeground)
                                }
                            } icon: {
                                Image(systemName: "books.vertical")
                            }
                        }
                        .accessibilityIdentifier("settings.library")
                        NavigationLink {
                            BackupsView(open: open)
                        } label: {
                            Label("settings.backups", systemImage: "clock.arrow.circlepath")
                        }
                        .accessibilityIdentifier("settings.backups")
                        NavigationLink {
                            CheckView(open: open)
                        } label: {
                            Label("settings.check", systemImage: "stethoscope")
                        }
                        .accessibilityIdentifier("settings.check")
                        NavigationLink {
                            ExportScreen(open: open)
                        } label: {
                            Label("settings.export", systemImage: "square.and.arrow.up.on.square")
                        }
                        .accessibilityIdentifier("settings.export")
                    } header: {
                        Text("settings.librarySection")
                    }
                }
                Section("settings.appSection") {
                    NavigationLink {
                        AppLockForm(lock: lock)
                    } label: {
                        HStack {
                            Label("appLock.title", systemImage: "lock.shield")
                            Spacer()
                            (lock.enabled ? Text("settings.on") : Text("settings.off"))
                                .foregroundStyle(EglTokens.mutedForeground)
                        }
                    }
                    .accessibilityIdentifier("settings.appLock")
                    NavigationLink {
                        DecryptArchiveScreen()
                    } label: {
                        Label("settings.decrypt", systemImage: "key")
                    }
                    .accessibilityIdentifier("settings.decrypt")
                    LabeledContent("settings.version") {
                        Text(verbatim: Self.version)
                    }
                    LabeledContent("settings.language") {
                        Text(verbatim: Locale.current.localizedString(forLanguageCode: Locale.current.language.languageCode?.identifier ?? "en") ?? "")
                    }
                    if let url = URL(string: UIApplication.openSettingsURLString) {
                        Link(destination: url) {
                            Label("settings.changeLanguage", systemImage: "globe")
                        }
                    }
                }
            }
            .navigationTitle("settings.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                        .accessibilityIdentifier("settings.done")
                }
            }
        }
    }

    static var version: String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "0.0.0"
        let build = info?["CFBundleVersion"] as? String ?? "0"
        return "\(short) (\(build))"
    }
}

/// The library's name, where it lives and what it takes on disk; rebuilding its thumbnails.
struct LibraryInfoView: View {
    let open: OpenLibrary
    @State private var name = ""
    @State private var stats: LibraryStats? = nil
    @State private var rebuilding = false

    var body: some View {
        Form {
            Section("settings.name") {
                TextField("settings.name", text: $name)
                    .submitLabel(.done)
                    .onSubmit(rename)
                    .disabled(open.readOnly)
                    .accessibilityIdentifier("settings.name")
            }
            Section("settings.location") {
                Text(verbatim: open.url.path)
                    .font(.footnote)
                    .foregroundStyle(EglTokens.mutedForeground)
                    .textSelection(.enabled)
                if open.readOnly {
                    Label("library.readOnly", systemImage: "lock")
                        .foregroundStyle(EglTokens.mutedForeground)
                }
            }
            Section("settings.storage") {
                if let stats {
                    bytes("settings.originals", stats.originals)
                    bytes("settings.thumbnails", stats.thumbnails)
                    bytes("settings.database", stats.database)
                    bytes("settings.backupsSize", stats.backups)
                    bytes("settings.total", stats.total)
                } else {
                    ProgressView()
                }
            }
            if !open.readOnly {
                Section {
                    Button("settings.rebuild") { rebuilding = true }
                        .accessibilityIdentifier("settings.rebuild")
                } footer: {
                    Text("settings.rebuildHint")
                }
            }
        }
        .navigationTitle("settings.libraryTitle")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            name = open.info.name
            stats = await open.perform { try $0.stats() }
        }
        .onDisappear(perform: rename)
        .confirmationDialog("settings.rebuildConfirm", isPresented: $rebuilding, titleVisibility: .visible) {
            Button("settings.rebuild") {
                Task {
                    if await open.perform({ try $0.generatePreviews(rebuild: true) }) != nil {
                        open.show(String(localized: "toast.rebuilding"), undoable: false)
                    }
                }
            }
        }
    }

    private func bytes(_ title: LocalizedStringKey, _ count: Int) -> some View {
        LabeledContent(title) {
            Text(verbatim: ByteCountFormatter.string(fromByteCount: Int64(count), countStyle: .file))
                .monospacedDigit()
        }
    }

    private func rename() {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !open.readOnly, !name.isEmpty, name != open.info.name else { return }
        Task { _ = await open.rename(to: name) }
    }
}

/// The library's database backups: back up now, or put one back.
struct BackupsView: View {
    let open: OpenLibrary
    @State private var backups: [Backup] = []
    @State private var restoring: Backup? = nil
    @State private var working = false

    var body: some View {
        List {
            if !open.readOnly {
                Section {
                    Button {
                        Task { await backUp() }
                    } label: {
                        Label("backups.now", systemImage: "plus.circle")
                    }
                    .disabled(working)
                    .accessibilityIdentifier("backups.now")
                } footer: {
                    Text("backups.hint")
                }
            }
            Section {
                ForEach(backups) { backup in
                    Button {
                        restoring = backup
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(Date(timeIntervalSince1970: Double(backup.createdAt) / 1000), format: .dateTime.year().month().day().hour().minute())
                                .foregroundStyle(EglTokens.foreground)
                            HStack {
                                BackupKindLabel(kind: backup.kind)
                                Text(verbatim: "· " + ByteCountFormatter.string(fromByteCount: Int64(backup.size), countStyle: .file))
                            }
                            .font(.caption)
                            .foregroundStyle(EglTokens.mutedForeground)
                        }
                    }
                    .disabled(open.readOnly || working)
                    .accessibilityIdentifier("backup.\(backup.name)")
                }
            }
        }
        .overlay {
            if backups.isEmpty, !working {
                ContentUnavailableView("backups.none", systemImage: "clock.arrow.circlepath")
            }
        }
        .navigationTitle("settings.backups")
        .navigationBarTitleDisplayMode(.inline)
        .task { await reload() }
        .confirmationDialog(
            "backups.restoreTitle",
            isPresented: Binding(get: { restoring != nil }, set: { if !$0 { restoring = nil } }),
            titleVisibility: .visible,
            presenting: restoring
        ) { backup in
            Button("backups.restore", role: .destructive) {
                Task {
                    working = true
                    _ = await open.restoreBackup(backup.name)
                    await reload()
                    working = false
                }
            }
            .accessibilityIdentifier("backups.restoreConfirm")
        } message: { _ in
            Text("backups.restoreHint")
        }
    }

    private func backUp() async {
        working = true
        if await open.perform({ try $0.backUpNow() }) != nil {
            open.show(String(localized: "toast.backedUp"), undoable: false)
        }
        await reload()
        working = false
    }

    private func reload() async {
        backups = await open.perform { try $0.backups() } ?? []
    }
}

private struct BackupKindLabel: View {
    let kind: BackupKind?

    var body: some View {
        switch kind {
        case .daily: Text("backups.daily")
        case .manual: Text("backups.manual")
        case .beforeMigration: Text("backups.beforeMigration")
        case .beforeRestore: Text("backups.beforeRestore")
        case nil: Text("backups.other")
        }
    }
}

/// Check Library and Repair: the database, missing originals, files nothing points at.
struct CheckView: View {
    let open: OpenLibrary
    @State private var result: LibraryCheck? = nil
    @State private var working = false
    @State private var removeOrphans = true
    @State private var removeMissing = false

    var body: some View {
        Form {
            Section {
                Button {
                    Task { await run { try $0.check() } }
                } label: {
                    Label("check.run", systemImage: "stethoscope")
                }
                .disabled(working)
                .accessibilityIdentifier("check.run")
            } footer: {
                Text("check.hint")
            }
            if working {
                Section { ProgressView().frame(maxWidth: .infinity) }
            }
            if let result {
                Section("check.result") {
                    LabeledContent("check.items") { Text(result.items, format: .number) }
                    LabeledContent("check.database") {
                        result.databaseOk ? Text("check.ok") : Text("check.problems")
                    }
                    ForEach(result.databaseProblems, id: \.self) { problem in
                        Text(verbatim: problem).font(.caption).foregroundStyle(.red)
                    }
                    LabeledContent("check.missing") { Text(result.missingCount, format: .number) }
                    LabeledContent("check.orphans") {
                        Text(verbatim: "\(result.orphanCount) · " + ByteCountFormatter.string(fromByteCount: Int64(result.orphanBytes), countStyle: .file))
                    }
                    LabeledContent("check.orphanThumbnails") { Text(result.orphanThumbnails, format: .number) }
                    if result.isClean {
                        Label("check.clean", systemImage: "checkmark.seal")
                            .foregroundStyle(.green)
                            .accessibilityIdentifier("check.clean")
                    }
                }
                if !result.isClean, !open.readOnly {
                    Section {
                        Toggle("check.removeOrphans", isOn: $removeOrphans)
                        Toggle("check.removeMissing", isOn: $removeMissing)
                        Button("check.repair", role: .destructive) {
                            let options = RepairOptions(orphans: removeOrphans, missing: removeMissing)
                            Task { await run { try $0.repair(options) } }
                        }
                        .disabled(working || (!removeOrphans && !removeMissing))
                    } footer: {
                        Text("check.repairHint")
                    }
                }
            }
        }
        .navigationTitle("settings.check")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func run(_ work: @escaping @Sendable (Library) throws -> LibraryCheck) async {
        working = true
        result = await open.perform(work)
        working = false
        await open.refresh()
    }
}
