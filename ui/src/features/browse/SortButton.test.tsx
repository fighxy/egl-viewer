import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLocale } from "@/i18n";
import { SortButton } from "./SortButton";

describe("SortButton", () => {
  beforeEach(() => {
    setAppLocale("en");
  });

  it("shows English sort labels by default", async () => {
    const user = userEvent.setup();
    render(<SortButton sort="added" ascending onSort={vi.fn()} onAscending={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Sort by Date Added" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sort by Date Added" }));
    expect(await screen.findByRole("menuitemradio", { name: "Title" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Ascending" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Descending" })).toBeInTheDocument();
  });

  it("localizes the control and menu under Russian", async () => {
    const user = userEvent.setup();
    setAppLocale("ru");
    render(<SortButton sort="added" ascending onSort={vi.fn()} onAscending={vi.fn()} />);
    const trigger = screen.getByRole("button", { name: "Сортировка: Дата добавления" });
    expect(trigger).toHaveTextContent("Дата добавления");
    await user.click(trigger);
    expect(await screen.findByRole("menuitemradio", { name: "Название" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Расширение" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Размер файла" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Размеры" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Оценка" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Теги" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "Случайно" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "По возрастанию" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "По убыванию" })).toBeInTheDocument();
  });
});
