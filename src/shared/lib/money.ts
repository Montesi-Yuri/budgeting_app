// Amounts are integer minor units of the single Currency (e.g. cents).
// They are never floats: formatting and parsing go through strings/BigInt.

/** How many decimals the Currency has (EUR 2, JPY 0, ...). */
export function minorDigits(currency: string): number {
	return new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions()
		.maximumFractionDigits ?? 2
}

/** Minor units as a plain decimal string for an input field: -35000 EUR -> "-350.00". */
export function toAmountInput(minor: number, currency: string): string {
	const digits = minorDigits(currency)
	const negative = minor < 0
	const units = (negative ? -BigInt(minor) : BigInt(minor)).toString().padStart(digits + 1, '0')
	const whole = units.slice(0, units.length - digits)
	const fraction = units.slice(units.length - digits)
	return `${negative ? '-' : ''}${whole}${digits > 0 ? '.' + fraction : ''}`
}

/** Format integer minor units in the Currency, e.g. 125050 EUR -> "€1,250.50". */
export function formatMinor(minor: number, currency: string, locale?: string): string {
	const formatter = new Intl.NumberFormat(locale, { style: 'currency', currency })
	// Intl accepts decimal strings exactly (no float rounding).
	return formatter.format(toAmountInput(minor, currency) as unknown as number)
}

export type ParsedAmount = { ok: true, minor: number } | { ok: false, error: string }

/**
 * Parse an amount typed by the user into integer minor units.
 * Accepts "-1250.50", "1250,5", "1,250.50", "1.250,50". With a single
 * separator it is the decimal separator, so ambiguous input such as "1.234"
 * (in a 2-decimal Currency) is rejected rather than guessed.
 */
export function parseAmount(text: string, currency: string): ParsedAmount {
	const digits = minorDigits(currency)
	const fail = (error: string): ParsedAmount => ({ ok: false, error })
	const match = /^([+-]?)([\d.,]+)$/.exec(text.replace(/\s/g, ''))
	if (!match) return fail('Enter an amount such as 1250.50')
	const [, sign, body] = match

	const lastDot = body.lastIndexOf('.')
	const lastComma = body.lastIndexOf(',')
	let whole = body
	let fraction = ''
	if (lastDot >= 0 && lastComma >= 0) {
		const decimalSep = lastDot > lastComma ? '.' : ','
		const thousandsSep = decimalSep === '.' ? ',' : '.'
		const cut = body.lastIndexOf(decimalSep)
		whole = body.slice(0, cut)
		fraction = body.slice(cut + 1)
		const groups = whole.split(thousandsSep)
		const grouped = /^\d{1,3}$/.test(groups[0]) && groups.slice(1).every((g) => /^\d{3}$/.test(g))
		if (!grouped) return fail('The thousands separators look wrong')
		whole = groups.join('')
	} else if (lastDot >= 0 || lastComma >= 0) {
		const parts = body.split(lastDot >= 0 ? '.' : ',')
		if (parts.length !== 2) return fail('Use one decimal separator');
		[whole, fraction] = parts
	}

	if (!/^\d+$/.test(whole) && !(whole === '' && fraction !== '')) return fail('Enter an amount such as 1250.50')
	if (!/^\d*$/.test(fraction)) return fail('Enter an amount such as 1250.50')
	if (fraction.length > digits) {
		return fail(digits === 0 ? `${currency} has no decimals` : `Use at most ${digits} decimals`)
	}

	const units = BigInt((whole || '0') + fraction.padEnd(digits, '0'))
	const minor = Number(sign === '-' ? -units : units)
	if (!Number.isSafeInteger(minor)) return fail('That amount is too large')
	return { ok: true, minor }
}
