import { ImportPreview, ImportProfile } from '@/shared/api'

type Props = {
  profile: ImportProfile
  /** The preview of the chosen file, for its column names. */
  preview: ImportPreview | null
  onChange: (profile: ImportProfile) => void
  onSave: () => void
  onCancel: (() => void) | null
  busy: boolean
}

const DATE_FORMATS = [
  { format: '%d/%m/%Y', example: '31/12/2026' },
  { format: '%d.%m.%Y', example: '31.12.2026' },
  { format: '%d-%m-%Y', example: '31-12-2026' },
  { format: '%Y-%m-%d', example: '2026-12-31' },
  { format: '%m/%d/%Y', example: '12/31/2026' },
  { format: '%d/%m/%y', example: '31/12/26' },
]

const DELIMITERS = [
  { value: ';', label: 'Semicolon ;' },
  { value: ',', label: 'Comma ,' },
  { value: '\t', label: 'Tab' },
]

const THOUSANDS = [
  { value: '', label: 'None' },
  { value: '.', label: 'Point .' },
  { value: ',', label: 'Comma ,' },
  { value: ' ', label: 'Space' },
  { value: '\'', label: 'Apostrophe \'' },
]

/** The setup step of an Import profile: the file's layout, column by column. */
export default function ImportProfileForm({ profile, preview, onChange, onSave, onCancel, busy }: Props) {
  const columns = preview?.columns ?? []
  const set = <K extends keyof ImportProfile>(key: K, value: ImportProfile[K]) => onChange({ ...profile, [key]: value })
  const knownFormat = DATE_FORMATS.some((f) => f.format === profile.date_format)

  function toggleDescription(column: string, on: boolean) {
    const rest = profile.description_columns.filter((c) => c !== column)
    set('description_columns', on ? [...rest, column] : rest)
  }

  return (
    <form
      className="card form"
      onSubmit={(e) => {
        e.preventDefault()
        onSave()
      }}
    >
      <h2>Import profile</h2>
      <p className="muted small">
        Describe this bank's CSV layout once. Later Imports into this Account use it again, and you can edit it at any time.
      </p>
      <label className="field">
        <span>Name</span>
        <input value={profile.name} placeholder="e.g. Bank current account CSV" onChange={(e) => set('name', e.target.value)} />
      </label>

      <div className="row">
        <label className="field">
          <span>Encoding</span>
          <select value={profile.encoding} onChange={(e) => set('encoding', e.target.value as ImportProfile['encoding'])}>
            <option value="utf-8">UTF-8</option>
            <option value="windows-1252">Windows-1252</option>
          </select>
        </label>
        <label className="field">
          <span>Separator</span>
          <select value={profile.delimiter} onChange={(e) => set('delimiter', e.target.value)}>
            {DELIMITERS.map((d) => (
              <option key={d.label} value={d.value}>
                {d.label}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Lines before the header</span>
          <input
            type="number"
            min={0}
            value={profile.skip_rows}
            onChange={(e) => set('skip_rows', Math.max(0, Number(e.target.value) || 0))}
          />
        </label>
      </div>

      <div className="row">
        <ColumnField label="Date column" columns={columns} value={profile.date_column} onChange={(v) => set('date_column', v ?? '')} />
        <label className="field">
          <span>Date format</span>
          <select
            value={knownFormat ? profile.date_format : 'custom'}
            onChange={(e) => set('date_format', e.target.value === 'custom' ? '' : e.target.value)}
          >
            {DATE_FORMATS.map((f) => (
              <option key={f.format} value={f.format}>
                {f.example}
              </option>
            ))}
            <option value="custom">Other…</option>
          </select>
        </label>
        {!knownFormat && (
          <label className="field">
            <span>Format (%d day, %m month, %Y year)</span>
            <input value={profile.date_format} onChange={(e) => set('date_format', e.target.value)} />
          </label>
        )}
      </div>

      <fieldset className="field">
        <span>Amounts</span>
        <label className="check">
          <input
            type="radio"
            checked={profile.amount_mode === 'signed'}
            onChange={() => set('amount_mode', 'signed')}
          />
          One column, negative = money out
        </label>
        <label className="check">
          <input type="radio" checked={profile.amount_mode === 'split'} onChange={() => set('amount_mode', 'split')} />
          Separate debit and credit columns
        </label>
      </fieldset>
      <div className="row">
        {profile.amount_mode === 'signed'
          ? (
              <ColumnField label="Amount column" columns={columns} value={profile.amount_column} onChange={(v) => set('amount_column', v)} />
            )
          : (
              <>
                <ColumnField label="Debit column (money out)" columns={columns} value={profile.debit_column} onChange={(v) => set('debit_column', v)} />
                <ColumnField label="Credit column (money in)" columns={columns} value={profile.credit_column} onChange={(v) => set('credit_column', v)} />
              </>
            )}
        <label className="field">
          <span>Decimal separator</span>
          <select value={profile.decimal_sep} onChange={(e) => set('decimal_sep', e.target.value as ',' | '.')}>
            <option value=",">Comma 1,50</option>
            <option value=".">Point 1.50</option>
          </select>
        </label>
        <label className="field">
          <span>Thousands separator</span>
          <select value={profile.thousands_sep ?? ''} onChange={(e) => set('thousands_sep', e.target.value === '' ? null : e.target.value)}>
            {THOUSANDS.map((t) => (
              <option key={t.label} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <fieldset className="field">
        <span>Description columns (joined in the order you tick them)</span>
        {columns.length === 0 && <span className="muted small">Choose a file to see its columns.</span>}
        {columns.map((c) => (
          <label key={c} className="check">
            <input
              type="checkbox"
              checked={profile.description_columns.includes(c)}
              onChange={(e) => toggleDescription(c, e.target.checked)}
            />
            {c}
          </label>
        ))}
        {profile.description_columns.length > 1 && (
          <span className="muted small">
            Joined as:
            {profile.description_columns.join(' + ')}
          </span>
        )}
      </fieldset>

      <ColumnField
        label="Balance column (optional, used to check the balance)"
        columns={columns}
        value={profile.balance_column}
        optional
        onChange={(v) => set('balance_column', v)}
      />

      <div className="actions">
        <button type="submit" className="primary" disabled={busy}>
          Save Import profile
        </button>
        {onCancel && (
          <button type="button" onClick={onCancel}>
            Cancel
          </button>
        )}
      </div>
    </form>
  )
}

function ColumnField({
  label,
  columns,
  value,
  optional,
  onChange,
}: {
  label: string
  columns: string[]
  value: string | null
  optional?: boolean
  onChange: (value: string | null) => void
}) {
  const options = value && !columns.includes(value) ? [value, ...columns] : columns
  return (
    <label className="field">
      <span>{label}</span>
      {columns.length === 0
        ? (
            <input value={value ?? ''} onChange={(e) => onChange(e.target.value === '' ? null : e.target.value)} />
          )
        : (
            <select value={value ?? ''} onChange={(e) => onChange(e.target.value === '' ? null : e.target.value)}>
              <option value="">{optional ? 'None' : 'Choose…'}</option>
              {options.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          )}
    </label>
  )
}
