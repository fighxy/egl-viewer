import { Fragment } from "react";
import {
  availability,
  COMMANDS,
  type Command,
  type CommandActions,
  type CommandState,
  labelOf,
  type SubmenuItem,
  visibleMenus,
} from "@/commands/registry";
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarRadioGroup,
  MenubarRadioItem,
  MenubarSeparator,
  MenubarShortcut,
  MenubarSub,
  MenubarSubContent,
  MenubarSubTrigger,
  MenubarTrigger,
} from "@/components/ui/menubar";

type Props = {
  state: CommandState;
  actions: CommandActions;
  commands?: readonly Command[];
};

/** The app's own menu bar under the native title bar (docs/spec.md §7.1, D-025). */
export function AppMenu({ state, actions, commands = COMMANDS }: Props) {
  return (
    <Menubar aria-label="Menu" className="border-b bg-background">
      {visibleMenus(commands).map((menu) => (
        <MenubarMenu key={menu.id}>
          <MenubarTrigger>{menu.label}</MenubarTrigger>
          <MenubarContent>
            {menu.commands.map((command, index) => (
              <Fragment key={command.id}>
                {index > 0 && menu.commands[index - 1]?.group !== command.group && (
                  <MenubarSeparator />
                )}
                <MenuCommand command={command} state={state} actions={actions} />
              </Fragment>
            ))}
          </MenubarContent>
        </MenubarMenu>
      ))}
    </Menubar>
  );
}

function MenuCommand({
  command,
  state,
  actions,
}: {
  command: Command;
  state: CommandState;
  actions: CommandActions;
}) {
  const available = availability(command, state);
  const disabled = available !== true;
  const title = disabled ? available : undefined;
  const shortcut = command.shortcut && <MenubarShortcut>{command.shortcut}</MenubarShortcut>;
  const label = labelOf(command, state);

  if (command.submenu) {
    return (
      <MenubarSub>
        <MenubarSubTrigger disabled={disabled} title={title}>
          {command.label}
        </MenubarSubTrigger>
        <MenubarSubContent>
          <SubmenuGroups items={command.submenu(state)} actions={actions} />
        </MenubarSubContent>
      </MenubarSub>
    );
  }

  if (command.checked) {
    return (
      <MenubarCheckboxItem
        checked={command.checked(state)}
        disabled={disabled}
        title={title}
        onSelect={() => command.run?.(actions)}
      >
        {label}
        {shortcut}
      </MenubarCheckboxItem>
    );
  }

  return (
    <MenubarItem disabled={disabled} title={title} onSelect={() => command.run?.(actions)}>
      {label}
      {shortcut}
    </MenubarItem>
  );
}

/** Each group of a submenu is its own choice (radio items) or a plain list, between lines. */
function SubmenuGroups({ items, actions }: { items: SubmenuItem[]; actions: CommandActions }) {
  const groups: SubmenuItem[][] = [];
  for (const item of items) {
    const last = groups[groups.length - 1];
    if (last && (last[0]?.group ?? 0) === (item.group ?? 0)) last.push(item);
    else groups.push([item]);
  }
  return groups.map((group, index) => {
    const radio = group.some((item) => item.checked !== undefined);
    return (
      <Fragment key={group[0]?.id}>
        {index > 0 && <MenubarSeparator />}
        {radio ? (
          <MenubarRadioGroup value={group.find((item) => item.checked)?.id}>
            {group.map((item) => (
              <MenubarRadioItem key={item.id} value={item.id} onSelect={() => item.run(actions)}>
                {item.label}
              </MenubarRadioItem>
            ))}
          </MenubarRadioGroup>
        ) : (
          group.map((item) => (
            <MenubarItem key={item.id} title={item.title} onSelect={() => item.run(actions)}>
              {item.label}
            </MenubarItem>
          ))
        )}
      </Fragment>
    );
  });
}
