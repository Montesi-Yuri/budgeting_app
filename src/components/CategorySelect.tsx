import type { Category } from "../api";
import { categoryChoices } from "../categories";

type Props = {
  categories: Category[];
  value: string | null;
  onChange: (categoryId: string | null) => void;
  disabled?: boolean;
  "aria-label"?: string;
};

/** A Category picker grouped by kind; the empty choice is Uncategorised. */
export default function CategorySelect({ categories, value, onChange, disabled, ...rest }: Props) {
  // A Category deleted (e.g. on another Device) is not offered: show Uncategorised.
  const known = value !== null && categories.some((c) => c.id === value);
  return (
    <select
      value={known ? value : ""}
      onChange={(e) => onChange(e.target.value || null)}
      disabled={disabled}
      aria-label={rest["aria-label"]}
    >
      <option value="">Uncategorised</option>
      {categoryChoices(categories).map((group) => (
        <optgroup key={group.kind} label={group.label}>
          {group.options.map((o) => (
            <option key={o.id} value={o.id}>
              {o.label}
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );
}
