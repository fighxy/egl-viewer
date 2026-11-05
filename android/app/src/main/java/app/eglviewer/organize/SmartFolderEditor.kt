package app.eglviewer.organize

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.browse.BrowseFilters
import app.eglviewer.core.ExtensionCount
import app.eglviewer.core.JSONValue
import app.eglviewer.core.SmartFolder
import app.eglviewer.core.SmartFolderChange
import app.eglviewer.core.Tag
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** What the smart folder editor works on. */
sealed class SmartEditing {
    data class Create(val parent: String?, val conditions: JSONValue?) : SmartEditing()
    data class Edit(val folder: SmartFolder) : SmartEditing()
}

@Composable
fun SmartFolderEditorDialog(
    open: OpenLibrary,
    editing: SmartEditing,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var parent by remember { mutableStateOf<String?>(null) }
    var conditions by remember { mutableStateOf<List<SmartCondition>>(emptyList()) }
    var vocabulary by remember { mutableStateOf<List<Tag>>(emptyList()) }
    var extensions by remember { mutableStateOf<List<ExtensionCount>>(emptyList()) }

    LaunchedEffect(editing) {
        when (editing) {
            is SmartEditing.Create -> {
                parent = editing.parent
                conditions = SmartRules.read(editing.conditions)
                name = ""
            }
            is SmartEditing.Edit -> {
                name = editing.folder.name
                conditions = SmartRules.read(editing.folder.conditions)
            }
        }
        if (conditions.isEmpty()) {
            conditions = listOf(SmartRules.newCondition())
        }
        vocabulary = withContext(Dispatchers.IO) {
            runCatching { open.library.tags() }.getOrDefault(emptyList())
        }
        extensions = withContext(Dispatchers.IO) {
            runCatching { open.library.extensions() }.getOrDefault(emptyList())
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                if (editing is SmartEditing.Create) {
                    stringResource(R.string.smart_new_title)
                } else {
                    stringResource(R.string.smart_edit_title)
                },
            )
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.smart_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (editing is SmartEditing.Create && open.smartFolders.isNotEmpty()) {
                    ParentPicker(
                        parent = parent,
                        folders = open.smartFolderList,
                        onParent = { parent = it },
                    )
                }
                conditions.forEachIndexed { index, condition ->
                    ConditionBlock(
                        condition = condition,
                        vocabulary = vocabulary,
                        extensions = extensions,
                        folders = open.pickableFolders,
                        removable = conditions.size > 1,
                        onChange = { updated ->
                            conditions = conditions.toMutableList().also { it[index] = updated }
                        },
                        onRemove = {
                            conditions = conditions.toMutableList().also { it.removeAt(index) }
                        },
                    )
                    HorizontalDivider(color = EglTokens.Grid)
                }
                TextButton(
                    onClick = {
                        conditions = conditions + SmartRules.newCondition()
                    },
                ) {
                    Text(stringResource(R.string.smart_add_condition))
                }
                Text(
                    text = stringResource(R.string.smart_conditions_hint),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = name.trim().isNotEmpty(),
                onClick = {
                    val trimmed = name.trim()
                    val rules = SmartRules.write(conditions)
                    val change = when (editing) {
                        is SmartEditing.Create -> SmartFolderChange.Create(
                            name = trimmed,
                            parent = parent,
                            conditions = rules,
                        )
                        is SmartEditing.Edit -> SmartFolderChange.Update(
                            id = editing.folder.id,
                            name = trimmed,
                            conditions = rules,
                        )
                    }
                    scope.launch {
                        if (open.saveSmartFolder(change)) onDismiss()
                    }
                },
            ) { Text(stringResource(R.string.common_save)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
private fun ParentPicker(
    parent: String?,
    folders: List<app.eglviewer.browse.PickableSmart>,
    onParent: (String?) -> Unit,
) {
    var openMenu by remember { mutableStateOf(false) }
    val label = when (parent) {
        null -> stringResource(R.string.smart_top_level)
        else -> folders.firstOrNull { it.folder.id == parent }?.folder?.name
            ?: stringResource(R.string.smart_top_level)
    }
    Column {
        Text(stringResource(R.string.smart_parent), fontSize = 13.sp, color = EglTokens.MutedForeground)
        TextButton(onClick = { openMenu = true }) { Text(label) }
        DropdownMenu(expanded = openMenu, onDismissRequest = { openMenu = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.smart_top_level)) },
                onClick = {
                    onParent(null)
                    openMenu = false
                },
            )
            folders.forEach { entry ->
                DropdownMenuItem(
                    text = {
                        Text("  ".repeat(entry.depth) + entry.folder.name)
                    },
                    onClick = {
                        onParent(entry.folder.id)
                        openMenu = false
                    },
                )
            }
        }
    }
}

@Composable
private fun ConditionBlock(
    condition: SmartCondition,
    vocabulary: List<Tag>,
    extensions: List<ExtensionCount>,
    folders: List<app.eglviewer.browse.PickableFolder>,
    removable: Boolean,
    onChange: (SmartCondition) -> Unit,
    onRemove: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = !condition.any,
                onClick = { onChange(condition.copy(fields = condition.fields.toMutableMap()).also { it.any = false }) },
                label = { Text(stringResource(R.string.smart_match_all)) },
            )
            FilterChip(
                selected = condition.any,
                onClick = { onChange(condition.copy(fields = condition.fields.toMutableMap()).also { it.any = true }) },
                label = { Text(stringResource(R.string.smart_match_any)) },
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.smart_negate), modifier = Modifier.weight(1f))
            Switch(
                checked = condition.negated,
                onCheckedChange = {
                    onChange(condition.copy(fields = condition.fields.toMutableMap()).also { c -> c.negated = it })
                },
            )
        }
        condition.rules.forEachIndexed { index, rule ->
            RuleBlock(
                rule = rule,
                vocabulary = vocabulary,
                extensions = extensions,
                folders = folders,
                onChange = { updated ->
                    val rules = condition.rules.toMutableList()
                    rules[index] = updated
                    onChange(condition.copy(rules = rules))
                },
                onRemove = {
                    val rules = condition.rules.toMutableList()
                    rules.removeAt(index)
                    onChange(condition.copy(rules = rules))
                },
            )
        }
        TextButton(
            onClick = {
                onChange(
                    condition.copy(
                        rules = (condition.rules + SmartRules.newRule("tags")).toMutableList(),
                    ),
                )
            },
        ) {
            Text(stringResource(R.string.smart_add_rule))
        }
        if (removable) {
            TextButton(onClick = onRemove) {
                Text(stringResource(R.string.smart_remove_condition))
            }
        }
    }
}

@Composable
private fun RuleBlock(
    rule: SmartRule,
    vocabulary: List<Tag>,
    extensions: List<ExtensionCount>,
    folders: List<app.eglviewer.browse.PickableFolder>,
    onChange: (SmartRule) -> Unit,
    onRemove: () -> Unit,
) {
    if (!rule.editable) {
        Column(modifier = Modifier.padding(vertical = 4.dp)) {
            Text(stringResource(R.string.smart_unsupported), color = EglTokens.MutedForeground, fontSize = 13.sp)
            Text(
                text = "${rule.property} ${rule.method}",
                color = EglTokens.MutedForeground,
                fontSize = 12.sp,
                fontFamily = FontFamily.Monospace,
            )
            TextButton(onClick = onRemove) {
                Text(stringResource(R.string.smart_remove_rule))
            }
        }
        return
    }
    val kind = rule.kind ?: return
    var propertyMenu by remember { mutableStateOf(false) }
    var methodMenu by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(vertical = 4.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = { propertyMenu = true }) {
                Text(propertyLabel(rule.property))
            }
            DropdownMenu(expanded = propertyMenu, onDismissRequest = { propertyMenu = false }) {
                SmartRules.properties.forEach { entry ->
                    DropdownMenuItem(
                        text = { Text(propertyLabel(entry.property)) },
                        onClick = {
                            onChange(SmartRules.withProperty(rule, entry.property))
                            propertyMenu = false
                        },
                    )
                }
            }
            TextButton(onClick = { methodMenu = true }) {
                Text(methodLabel(rule.method, kind))
            }
            DropdownMenu(expanded = methodMenu, onDismissRequest = { methodMenu = false }) {
                SmartRules.methodsOf(kind).forEach { method ->
                    DropdownMenuItem(
                        text = { Text(methodLabel(method, kind)) },
                        onClick = {
                            onChange(SmartRules.withMethod(rule, method))
                            methodMenu = false
                        },
                    )
                }
            }
        }
        if (SmartRules.needsValue(rule.method)) {
            RuleValue(
                rule = rule,
                kind = kind,
                vocabulary = vocabulary,
                extensions = extensions,
                folders = folders,
                onChange = onChange,
            )
        }
        TextButton(onClick = onRemove) {
            Text(stringResource(R.string.smart_remove_rule))
        }
    }
}

@Composable
private fun RuleValue(
    rule: SmartRule,
    kind: RuleKind,
    vocabulary: List<Tag>,
    extensions: List<ExtensionCount>,
    folders: List<app.eglviewer.browse.PickableFolder>,
    onChange: (SmartRule) -> Unit,
) {
    when (kind) {
        RuleKind.SET -> {
            val chosen = rule.value?.strings.orEmpty()
            if (rule.property == "tags") {
                Column {
                    chosen.forEach { name ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(name, modifier = Modifier.weight(1f))
                            TextButton(
                                onClick = {
                                    val next = chosen.filter { it != name }
                                    onChange(
                                        rule.copy(fields = rule.fields.toMutableMap()).also {
                                            it.value = JSONValue.Array(next.map { n -> JSONValue.Text(n) })
                                        },
                                    )
                                },
                            ) { Text(stringResource(R.string.smart_remove_value)) }
                        }
                    }
                    var newTag by remember { mutableStateOf("") }
                    OutlinedTextField(
                        value = newTag,
                        onValueChange = { newTag = it },
                        label = { Text(stringResource(R.string.tags_add)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    val suggestions = vocabulary.map { it.name }
                        .filter { it !in chosen && (newTag.isBlank() || it.contains(newTag, ignoreCase = true)) }
                        .take(8)
                    suggestions.forEach { suggestion ->
                        Text(
                            text = suggestion,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    onChange(
                                        rule.copy(fields = rule.fields.toMutableMap()).also {
                                            it.value = JSONValue.Array(
                                                (chosen + suggestion).map { n -> JSONValue.Text(n) },
                                            )
                                        },
                                    )
                                    newTag = ""
                                }
                                .padding(vertical = 8.dp),
                        )
                    }
                    if (newTag.isNotBlank() && newTag !in chosen) {
                        TextButton(
                            onClick = {
                                onChange(
                                    rule.copy(fields = rule.fields.toMutableMap()).also {
                                        it.value = JSONValue.Array(
                                            (chosen + newTag.trim()).map { n -> JSONValue.Text(n) },
                                        )
                                    },
                                )
                                newTag = ""
                            },
                        ) { Text(stringResource(R.string.tags_add)) }
                    }
                }
            } else {
                Column {
                    chosen.forEach { id ->
                        val label = folders.firstOrNull { it.folder.id == id }?.folder?.name ?: id
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(label, modifier = Modifier.weight(1f))
                            TextButton(
                                onClick = {
                                    val next = chosen.filter { it != id }
                                    onChange(
                                        rule.copy(fields = rule.fields.toMutableMap()).also {
                                            it.value = JSONValue.Array(next.map { n -> JSONValue.Text(n) })
                                        },
                                    )
                                },
                            ) { Text(stringResource(R.string.smart_remove_value)) }
                        }
                    }
                    var menu by remember { mutableStateOf(false) }
                    TextButton(onClick = { menu = true }) {
                        Text(stringResource(R.string.smart_add_folder))
                    }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        folders.filter { it.folder.id !in chosen }.forEach { entry ->
                            DropdownMenuItem(
                                text = { Text("  ".repeat(entry.depth) + entry.folder.name) },
                                onClick = {
                                    onChange(
                                        rule.copy(fields = rule.fields.toMutableMap()).also {
                                            it.value = JSONValue.Array(
                                                (chosen + entry.folder.id).map { n -> JSONValue.Text(n) },
                                            )
                                        },
                                    )
                                    menu = false
                                },
                            )
                        }
                    }
                }
            }
        }
        RuleKind.TEXT -> {
            OutlinedTextField(
                value = rule.value?.string.orEmpty(),
                onValueChange = {
                    onChange(
                        rule.copy(fields = rule.fields.toMutableMap()).also { r ->
                            r.value = JSONValue.Text(it)
                        },
                    )
                },
                label = { Text(stringResource(R.string.smart_text)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        RuleKind.TYPE -> {
            var menu by remember { mutableStateOf(false) }
            val current = rule.value?.string.orEmpty()
            TextButton(onClick = { menu = true }) {
                Text(if (current == "image") stringResource(R.string.smart_images) else current.uppercase())
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.smart_images)) },
                    onClick = {
                        onChange(rule.copy(fields = rule.fields.toMutableMap()).also {
                            it.value = JSONValue.Text("image")
                        })
                        menu = false
                    },
                )
                extensions.forEach { entry ->
                    DropdownMenuItem(
                        text = { Text(entry.ext.uppercase()) },
                        onClick = {
                            onChange(rule.copy(fields = rule.fields.toMutableMap()).also {
                                it.value = JSONValue.Text(entry.ext)
                            })
                            menu = false
                        },
                    )
                }
            }
        }
        RuleKind.RATING -> {
            var menu by remember { mutableStateOf(false) }
            TextButton(onClick = { menu = true }) {
                Text(rule.value?.string ?: "5")
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.rating_none)) },
                    onClick = {
                        onChange(rule.copy(fields = rule.fields.toMutableMap()).also {
                            it.value = JSONValue.Text("none")
                        })
                        menu = false
                    },
                )
                (1..5).forEach { stars ->
                    DropdownMenuItem(
                        text = { Text("★".repeat(stars)) },
                        onClick = {
                            onChange(rule.copy(fields = rule.fields.toMutableMap()).also {
                                it.value = JSONValue.Text(stars.toString())
                            })
                            menu = false
                        },
                    )
                }
            }
        }
        RuleKind.SHAPE -> {
            var menu by remember { mutableStateOf(false) }
            TextButton(onClick = { menu = true }) {
                Text(rule.value?.string ?: "landscape")
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                BrowseFilters.Shape.entries.forEach { shape ->
                    DropdownMenuItem(
                        text = { Text(shape.wire) },
                        onClick = {
                            onChange(rule.copy(fields = rule.fields.toMutableMap()).also {
                                it.value = JSONValue.Text(shape.wire)
                            })
                            menu = false
                        },
                    )
                }
            }
        }
        RuleKind.DATE, RuleKind.NUMBER -> {
            val numbers = rule.value?.numbers.orEmpty()
            OutlinedTextField(
                value = (numbers.firstOrNull() ?: 0.0).let {
                    if (it == kotlin.math.round(it)) it.toLong().toString() else it.toString()
                },
                onValueChange = { text ->
                    val n = text.toDoubleOrNull() ?: return@OutlinedTextField
                    val next = if (rule.method == "between" && numbers.size > 1) {
                        listOf(n, numbers[1])
                    } else {
                        listOf(n)
                    }
                    onChange(
                        rule.copy(fields = rule.fields.toMutableMap()).also { r ->
                            r.value = JSONValue.Array(next.map { JSONValue.Number(it) })
                        },
                    )
                },
                label = {
                    Text(
                        if (kind == RuleKind.DATE && rule.method == "within") {
                            stringResource(R.string.smart_days)
                        } else {
                            stringResource(R.string.smart_number)
                        },
                    )
                },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            if (rule.method == "between") {
                OutlinedTextField(
                    value = (numbers.getOrNull(1) ?: numbers.firstOrNull() ?: 0.0).let {
                        if (it == kotlin.math.round(it)) it.toLong().toString() else it.toString()
                    },
                    onValueChange = { text ->
                        val n = text.toDoubleOrNull() ?: return@OutlinedTextField
                        val first = numbers.firstOrNull() ?: 0.0
                        onChange(
                            rule.copy(fields = rule.fields.toMutableMap()).also { r ->
                                r.value = JSONValue.Array(
                                    listOf(JSONValue.Number(first), JSONValue.Number(n)),
                                )
                            },
                        )
                    },
                    label = { Text(stringResource(R.string.smart_to)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (kind == RuleKind.NUMBER && rule.property == "fileSize") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = rule.fields["unit"]?.string != "kb",
                        onClick = {
                            onChange(rule.copy(fields = rule.fields.toMutableMap().also {
                                it["unit"] = JSONValue.Text("mb")
                            }))
                        },
                        label = { Text("MB") },
                    )
                    FilterChip(
                        selected = rule.fields["unit"]?.string == "kb",
                        onClick = {
                            onChange(rule.copy(fields = rule.fields.toMutableMap().also {
                                it["unit"] = JSONValue.Text("kb")
                            }))
                        },
                        label = { Text("KB") },
                    )
                }
            }
        }
    }
}

@Composable
private fun propertyLabel(property: String): String = stringResource(
    when (property) {
        "tags" -> R.string.smart_property_tags
        "folders" -> R.string.smart_property_folders
        "name" -> R.string.smart_property_name
        "annotation" -> R.string.smart_property_notes
        "url" -> R.string.smart_property_link
        "type" -> R.string.smart_property_type
        "rating" -> R.string.smart_property_rating
        "shape" -> R.string.smart_property_shape
        "createTime" -> R.string.smart_property_added
        "mtime" -> R.string.smart_property_modified
        "btime" -> R.string.smart_property_created
        "width" -> R.string.smart_property_width
        "height" -> R.string.smart_property_height
        else -> R.string.smart_property_file_size
    },
)

@Composable
private fun methodLabel(method: String, kind: RuleKind): String = stringResource(
    when {
        method == "union" -> R.string.smart_method_any_of
        method == "intersection" -> R.string.smart_method_all_of
        method == "equal" && kind == RuleKind.SET -> R.string.smart_method_exactly
        method == "identity" -> R.string.smart_method_none_of
        method == "empty" -> R.string.smart_method_is_empty
        method == "not-empty" -> R.string.smart_method_is_not_empty
        method == "contain" -> R.string.smart_method_contains
        method == "uncontain" -> R.string.smart_method_does_not_contain
        method == "startWith" -> R.string.smart_method_starts_with
        method == "endWith" -> R.string.smart_method_ends_with
        method == "unequal" -> R.string.smart_method_is_not
        method == "on" -> R.string.smart_method_on
        method == "before" -> R.string.smart_method_before
        method == "after" -> R.string.smart_method_after
        method == "between" -> R.string.smart_method_between
        method == "within" -> R.string.smart_method_in_the_last
        method == ">=" -> R.string.smart_method_at_least
        method == "<=" -> R.string.smart_method_at_most
        method == ">" -> R.string.smart_method_more_than
        method == "<" -> R.string.smart_method_less_than
        else -> R.string.smart_method_is
    },
)
