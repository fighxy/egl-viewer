import { FOLDER_COLORS } from "./tree";

/** The eight library colours and "No colour": lock labels and tag groups. */
export function ColourPicker({
  legend,
  value,
  onChange,
}: {
  legend: string;
  value: string | null;
  onChange: (color: string | null) => void;
}) {
  return (
    <fieldset className="grid gap-1">
      <legend className="mb-1 text-xs text-muted-foreground">{legend}</legend>
      <div className="flex flex-wrap gap-1.5">
        <button
          type="button"
          aria-label="No colour"
          aria-pressed={value === null}
          className="size-5 rounded-full border aria-pressed:ring-2 aria-pressed:ring-ring"
          onClick={() => onChange(null)}
        />
        {Object.entries(FOLDER_COLORS).map(([name, css]) => (
          <button
            key={name}
            type="button"
            aria-label={name}
            aria-pressed={value === name}
            className="size-5 rounded-full aria-pressed:ring-2 aria-pressed:ring-ring aria-pressed:ring-offset-1"
            style={{ background: css }}
            onClick={() => onChange(name)}
          />
        ))}
      </div>
    </fieldset>
  );
}
