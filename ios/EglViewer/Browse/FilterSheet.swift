import EglCore
import SwiftUI

/// The filters of a place: tags (any, all or none of them), file type, rating and shape. Each
/// change applies at once, as the desktop filter bar does.
struct FilterSheet: View {
    let model: BrowseModel
    @Environment(\.dismiss) private var dismiss
    @State private var tagSearch = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("filter.tags") {
                    if model.vocabulary.isEmpty {
                        Text("filter.noTags")
                            .foregroundStyle(EglTokens.mutedForeground)
                    } else {
                        Picker("filter.tagMode", selection: model.binding(\.filters.tagMode)) {
                            Text("filter.tagMode.any").tag(TagMode.any)
                            Text("filter.tagMode.all").tag(TagMode.all)
                            Text("filter.tagMode.none").tag(TagMode.none)
                        }
                        .pickerStyle(.segmented)
                        ForEach(shownTags, id: \.name) { tag in
                            Button {
                                model.toggleTag(tag.name)
                            } label: {
                                HStack {
                                    Text(verbatim: tag.name)
                                        .foregroundStyle(EglTokens.foreground)
                                    Spacer()
                                    if let count = tag.imageCount {
                                        Text(count, format: .number)
                                            .foregroundStyle(EglTokens.mutedForeground)
                                            .monospacedDigit()
                                    }
                                    if model.settings.filters.tags.contains(tag.name) {
                                        Image(systemName: "checkmark")
                                            .foregroundStyle(EglTokens.accent)
                                    }
                                }
                            }
                            .accessibilityAddTraits(
                                model.settings.filters.tags.contains(tag.name) ? .isSelected : []
                            )
                            .accessibilityIdentifier("filter.tag.\(tag.name)")
                        }
                    }
                }

                Section {
                    Picker("filter.type", selection: model.binding(\.filters.ext)) {
                        Text("filter.any").tag(String?.none)
                        ForEach(model.extensions, id: \.ext) { type in
                            Text(verbatim: "\(type.ext.uppercased()) (\(type.count))").tag(Optional(type.ext))
                        }
                    }
                    Picker("filter.rating", selection: model.binding(\.filters.rating)) {
                        Text("filter.any").tag(Int?.none)
                        Text("filter.unrated").tag(Optional(0))
                        ForEach(1...5, id: \.self) { stars in
                            Text(verbatim: String(repeating: "★", count: stars)).tag(Optional(stars))
                        }
                    }
                    Picker("filter.shape", selection: model.binding(\.filters.shape)) {
                        Text("filter.any").tag(BrowseFilters.Shape?.none)
                        ForEach(BrowseFilters.Shape.allCases, id: \.self) { shape in
                            Text(ShapeLabel.of(shape)).tag(Optional(shape))
                        }
                    }
                }
            }
            .searchable(text: $tagSearch, placement: .navigationBarDrawer, prompt: Text("filter.findTag"))
            .navigationTitle("filter.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("filter.reset") { model.resetFilters() }
                        .disabled(!model.settings.filters.isActive)
                        .accessibilityIdentifier("filter.reset")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                        .accessibilityIdentifier("filter.done")
                }
            }
            .task {
                model.wantsVocabulary = true
                await model.loadVocabulary()
            }
            .onDisappear { model.wantsVocabulary = false }
        }
        .presentationDetents([.medium, .large])
    }

    /// The tags matching the search field; the chosen ones stay listed.
    private var shownTags: [Tag] {
        let needle = tagSearch.trimmingCharacters(in: .whitespaces)
        guard !needle.isEmpty else { return model.vocabulary }
        return model.vocabulary.filter {
            $0.name.localizedCaseInsensitiveContains(needle) || model.settings.filters.tags.contains($0.name)
        }
    }
}

enum ShapeLabel {
    static func of(_ shape: BrowseFilters.Shape) -> LocalizedStringKey {
        switch shape {
        case .square: "shape.square"
        case .landscape: "shape.landscape"
        case .portrait: "shape.portrait"
        case .panoramicLandscape: "shape.panoramicLandscape"
        case .panoramicPortrait: "shape.panoramicPortrait"
        }
    }
}
