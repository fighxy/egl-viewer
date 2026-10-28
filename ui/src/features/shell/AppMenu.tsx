import { Fragment } from "react";
import {
  availability,
  COMMANDS,
  type Command,
  type CommandActions,
  type CommandState,
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

  if (command.submenu) {
    const items = command.submenu(state);
    const radio = items.some((item) => item.checked !== undefined);
    return (
      <MenubarSub>
        <MenubarSubTrigger disabled={disabled} title={title}>
          {command.label}
        </MenubarSubTrigger>
        <MenubarSubContent>
          {radio ? (
            <MenubarRadioGroup value={items.find((item) => item.checked)?.id}>
              {items.map((item) => (
                <MenubarRadioItem key={item.id} value={item.id} onSelect={() => item.run(actions)}>
                  {item.label}
                </MenubarRadioItem>
              ))}
            </MenubarRadioGroup>
          ) : (
            items.map((item) => (
              <MenubarItem key={item.id} title={item.title} onSelect={() => item.run(actions)}>
                {item.label}
              </MenubarItem>
            ))
          )}
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
        {command.label}
        {shortcut}
      </MenubarCheckboxItem>
    );
  }

  return (
    <MenubarItem disabled={disabled} title={title} onSelect={() => command.run?.(actions)}>
      {command.label}
      {shortcut}
    </MenubarItem>
  );
}
