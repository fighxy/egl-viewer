import EglCore
import SwiftUI

/// What the smart folder editor works on.
enum SmartEditing: Identifiable {
    /// A new smart folder under `parent`, perhaps with the rules of a view.
    case create(parent: String?, conditions: JSONValue?)
    case edit(SmartFolder)

    var id: String {
        switch self {
        case let .create(parent, _): "new.\(parent ?? "")"
        case let .edit(folder): folder.id
        }
    }
}

/// A smart folder's name and rules (D-097), as the desktop editor: conditions that all must
/// hold, each matching all or any of its rules, or negated. Rules this editor does not know
/// stay as they are and can only be removed.
struct SmartFolderEditor: View {
    let open: OpenLibrary
    let editing: SmartEditing
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var parent: String? = nil
    @State private var conditions: [SmartCondition] = []
    @State private var vocabulary: [Tag] = []
    @State private var extensions: [ExtensionCount] = []

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("smart.name", text: $name)
                        .accessibilityIdentifier("smart.name")
                    if case .create = editing, !open.smartFolders.isEmpty {
                        Picker("smart.parent", selection: $parent) {
                            Text("smart.topLevel").tag(String?.none)
                            ForEach(open.smartFolderList, id: \.folder.id) { entry in
                                Text(verbatim: String(repeating: "  ", count: entry.depth) + entry.folder.name)
                                    .tag(Optional(entry.folder.id))
                            }
                        }
                    }
                }
                ForEach($conditions) { $condition in
                    ConditionSection(
                        condition: $condition,
                        vocabulary: vocabulary,
                        extensions: extensions,
                        folders: open.pickableFolders,
                        removable: conditions.count > 1
                    ) {
                        conditions.removeAll { $0.id == condition.id }
                    }
                }
                Section {
                    Button {
                        conditions.append(SmartRules.newCondition())
                    } label: {
                        Label("smart.addCondition", systemImage: "plus.rectangle.on.rectangle")
                    }
                    .accessibilityIdentifier("smart.addCondition")
                } footer: {
                    Text("smart.conditionsHint")
                }
            }
            .navigationTitle(isNew ? Text("smart.newTitle") : Text("smart.editTitle"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.save", action: save)
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("smart.save")
                }
            }
            .task {
                switch editing {
                case let .create(parent, draft):
                    self.parent = parent
                    conditions = SmartRules.read(draft)
                case let .edit(folder):
                    name = folder.name
                    conditions = SmartRules.read(folder.conditions)
                }
                if conditions.isEmpty {
                    conditions = [SmartRules.newCondition()]
                }
                vocabulary = await open.perform { try $0.tags() } ?? []
                extensions = await open.perform { try $0.extensions() } ?? []
            }
        }
    }

    private var isNew: Bool {
        if case .create = editing { return true }
        return false
    }

    private func save() {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let rules = SmartRules.write(conditions)
        let change: SmartFolderChange = switch editing {
        case .create: .create(name: name, parent: parent, conditions: rules)
        case let .edit(folder): .update(id: folder.id, name: name, conditions: rules)
        }
        Task {
            if await open.saveSmartFolder(change) {
                dismiss()
            }
        }
    }
}

/// One condition: all or any of its rules, or the items that do not match them.
private struct ConditionSection: View {
    @Binding var condition: SmartCondition
    let vocabulary: [Tag]
    let extensions: [ExtensionCount]
    let folders: [(folder: Folder, depth: Int)]
    let removable: Bool
    let remove: () -> Void

    var body: some View {
        Section {
            Picker("smart.match", selection: $condition.any) {
                Text("smart.matchAll").tag(false)
                Text("smart.matchAny").tag(true)
            }
            .pickerStyle(.segmented)
            Toggle("smart.negate", isOn: $condition.negated)
            ForEach($condition.rules) { $rule in
                RuleEditor(rule: $rule, vocabulary: vocabulary, extensions: extensions, folders: folders)
                    .swipeActions {
                        Button("smart.removeRule", role: .destructive) {
                            condition.rules.removeAll { $0.id == rule.id }
                        }
                    }
            }
            Button {
                condition.rules.append(SmartRules.newRule("tags"))
            } label: {
                Label("smart.addRule", systemImage: "plus")
            }
            .accessibilityIdentifier("smart.addRule")
            if removable {
                Button("smart.removeCondition", role: .destructive, action: remove)
            }
        }
    }
}

/// One rule: property, method and a value editor of its kind.
private struct RuleEditor: View {
    @Binding var rule: SmartRule
    let vocabulary: [Tag]
    let extensions: [ExtensionCount]
    let folders: [(folder: Folder, depth: Int)]
    @State private var newTag = ""

    var body: some View {
        if !rule.editable {
            VStack(alignment: .leading, spacing: 2) {
                Text("smart.unsupported")
                    .foregroundStyle(EglTokens.mutedForeground)
                Text(verbatim: "\(rule.property) \(rule.method)")
                    .font(.caption.monospaced())
                    .foregroundStyle(EglTokens.mutedForeground)
            }
            .accessibilityIdentifier("smart.unsupportedRule")
        } else if let kind = rule.kind {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Picker("smart.property", selection: Binding(
                        get: { rule.property },
                        set: { rule = SmartRules.withProperty(rule, $0) }
                    )) {
                        ForEach(SmartRules.properties, id: \.property) { entry in
                            Text(SmartLabels.property(entry.property)).tag(entry.property)
                        }
                    }
                    .labelsHidden()
                    Picker("smart.method", selection: Binding(
                        get: { rule.method },
                        set: { rule = SmartRules.withMethod(rule, $0) }
                    )) {
                        ForEach(SmartRules.methods(of: kind), id: \.method) { entry in
                            Text(SmartLabels.method(entry.method, kind: kind)).tag(entry.method)
                        }
                    }
                    .labelsHidden()
                }
                if SmartRules.needsValue(rule.method) {
                    value(kind)
                }
            }
            .padding(.vertical, 2)
        }
    }

    @ViewBuilder
    private func value(_ kind: RuleKind) -> some View {
        switch kind {
        case .set where rule.property == "tags":
            let chosen = rule.value?.strings ?? []
            Chips(names: chosen) { name in
                setStrings(chosen.filter { $0 != name })
            }
            TagEntry(text: $newTag, vocabulary: vocabulary, taken: Set(chosen)) { name in
                setStrings(chosen + [name])
            }
        case .set:
            let chosen = rule.value?.strings ?? []
            Chips(names: chosen.map { id in folders.first { $0.folder.id == id }?.folder.name ?? id }) { name in
                setStrings(chosen.filter { id in (folders.first { $0.folder.id == id }?.folder.name ?? id) != name })
            }
            Menu {
                ForEach(folders.filter { !chosen.contains($0.folder.id) }, id: \.folder.id) { entry in
                    Button {
                        setStrings(chosen + [entry.folder.id])
                    } label: {
                        Text(verbatim: String(repeating: "  ", count: entry.depth) + entry.folder.name)
                    }
                }
            } label: {
                Label("smart.addFolder", systemImage: "folder.badge.plus")
            }
        case .text:
            TextField("smart.text", text: Binding(
                get: { rule.value?.string ?? "" },
                set: { rule.value = .string($0) }
            ))
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
        case .type:
            Picker("smart.value", selection: stringBinding) {
                Text("smart.images").tag("image")
                ForEach(extensions, id: \.ext) { entry in
                    Text(verbatim: entry.ext.uppercased()).tag(entry.ext)
                }
            }
        case .rating:
            Picker("smart.value", selection: stringBinding) {
                Text("filter.unrated").tag("none")
                ForEach(1...5, id: \.self) { stars in
                    Text(verbatim: String(repeating: "★", count: stars)).tag(String(stars))
                }
            }
        case .shape:
            Picker("smart.value", selection: stringBinding) {
                ForEach(BrowseFilters.Shape.allCases, id: \.self) { shape in
                    Text(ShapeLabel.of(shape)).tag(shape.rawValue)
                }
            }
        case .date:
            dateValue
        case .number:
            numberValue
        }
    }

    private var stringBinding: Binding<String> {
        Binding(get: { rule.value?.string ?? "" }, set: { rule.value = .string($0) })
    }

    private func setStrings(_ values: [String]) {
        rule.value = .array(values.map(JSONValue.string))
    }

    private func number(_ index: Int) -> Double {
        let numbers = rule.value?.numbers ?? []
        return index < numbers.count ? numbers[index] : 0
    }

    private func setNumber(_ index: Int, _ value: Double) {
        var numbers = rule.value?.numbers ?? []
        while numbers.count <= index { numbers.append(0) }
        numbers[index] = value
        rule.value = .array(numbers.map(JSONValue.number))
    }

    @ViewBuilder
    private var dateValue: some View {
        if rule.method == "within" {
            let days = max(Int(number(0)), 1)
            Stepper(value: Binding(get: { days }, set: { setNumber(0, Double($0)) }), in: 1...3650) {
                Text("smart.days \(days)")
            }
        } else {
            day(0)
            if rule.method == "between" {
                day(1)
            }
        }
    }

    private func day(_ index: Int) -> some View {
        DatePicker(
            selection: Binding(
                get: { Date(timeIntervalSince1970: number(index) / 1000) },
                set: { setNumber(index, Calendar.current.startOfDay(for: $0).timeIntervalSince1970 * 1000) }
            ),
            displayedComponents: .date
        ) {
            if rule.method == "between" {
                index == 0 ? Text("smart.from") : Text("smart.to")
            } else {
                Text("smart.date")
            }
        }
    }

    @ViewBuilder
    private var numberValue: some View {
        HStack {
            NumberField(value: Binding(get: { number(0) }, set: { setNumber(0, $0) }))
            if rule.method == "between" {
                Text(verbatim: "–")
                NumberField(value: Binding(get: { number(1) }, set: { setNumber(1, $0) }))
            }
            if rule.property == "fileSize" {
                Picker("smart.unit", selection: Binding(
                    get: { rule.fields["unit"]?.string == "kb" ? "kb" : "mb" },
                    set: { rule.fields["unit"] = .string($0) }
                )) {
                    Text(verbatim: "KB").tag("kb")
                    Text(verbatim: "MB").tag("mb")
                }
                .labelsHidden()
            } else {
                Text("smart.pixels")
                    .foregroundStyle(EglTokens.mutedForeground)
            }
        }
    }
}

private struct NumberField: View {
    @Binding var value: Double

    var body: some View {
        TextField("smart.number", value: $value, format: .number)
            .keyboardType(.decimalPad)
            .frame(maxWidth: 110)
    }
}

/// Chosen names with a way to remove each.
private struct Chips: View {
    let names: [String]
    let remove: (String) -> Void

    var body: some View {
        ForEach(names, id: \.self) { name in
            HStack {
                Text(verbatim: name)
                Spacer()
                Button {
                    remove(name)
                } label: {
                    Label("smart.removeValue", systemImage: "minus.circle")
                        .labelStyle(.iconOnly)
                }
                .buttonStyle(.borderless)
            }
        }
    }
}

/// Labels of the editor's properties and methods.
enum SmartLabels {
    static func property(_ property: String) -> LocalizedStringKey {
        switch property {
        case "tags": "smart.property.tags"
        case "folders": "smart.property.folders"
        case "name": "smart.property.name"
        case "annotation": "smart.property.notes"
        case "url": "smart.property.link"
        case "type": "smart.property.type"
        case "rating": "smart.property.rating"
        case "shape": "smart.property.shape"
        case "createTime": "smart.property.added"
        case "mtime": "smart.property.modified"
        case "btime": "smart.property.created"
        case "width": "smart.property.width"
        case "height": "smart.property.height"
        default: "smart.property.fileSize"
        }
    }

    static func method(_ method: String, kind: RuleKind) -> LocalizedStringKey {
        switch (method, kind) {
        case ("union", _): "smart.method.anyOf"
        case ("intersection", _): "smart.method.allOf"
        case ("equal", .set): "smart.method.exactly"
        case ("identity", _): "smart.method.noneOf"
        case ("empty", _): "smart.method.isEmpty"
        case ("not-empty", _): "smart.method.isNotEmpty"
        case ("contain", _): "smart.method.contains"
        case ("uncontain", _): "smart.method.doesNotContain"
        case ("startWith", _): "smart.method.startsWith"
        case ("endWith", _): "smart.method.endsWith"
        case ("unequal", _): "smart.method.isNot"
        case ("on", _): "smart.method.on"
        case ("before", _): "smart.method.before"
        case ("after", _): "smart.method.after"
        case ("between", _): "smart.method.between"
        case ("within", _): "smart.method.inTheLast"
        case (">=", _): "smart.method.atLeast"
        case ("<=", _): "smart.method.atMost"
        case (">", _): "smart.method.moreThan"
        case ("<", _): "smart.method.lessThan"
        default: "smart.method.is"
        }
    }
}
