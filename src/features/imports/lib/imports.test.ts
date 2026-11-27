import { describe, expect, it } from 'vitest'
import type { ImportProfile } from '@/shared/api'
import { guessColumns, guessDelimiter, importResultText } from './imports'

const draft: ImportProfile = {
	id: 'p',
	account_id: 'a',
	name: '',
	encoding: 'utf-8',
	delimiter: ';',
	skip_rows: 0,
	date_column: '',
	date_format: '%d/%m/%Y',
	amount_mode: 'signed',
	amount_column: null,
	debit_column: null,
	credit_column: null,
	decimal_sep: ',',
	thousands_sep: null,
	description_columns: [],
	balance_column: null,
}

describe('guessDelimiter picks the separator of a header line', () => {
	it.each([
		['Data;Descrizione;Importo', ';'],
		['Date,Description,Amount', ','],
		['Date\tDescription\tAmount', '\t'],
		['"Date","Payee, name",Amount', ','],
		['Data;"Descrizione, operazione";Importo', ';'],
	])('%j -> %j', (line, expected) => {
		expect(guessDelimiter(line)).toBe(expected)
	})
})

describe('guessColumns fills the columns it recognises', () => {
	it('recognises Italian headers with one signed amount', () => {
		const columns = ['Data contabile', 'Data valuta', 'Descrizione', 'Importo', 'Saldo']
		expect(guessColumns(draft, columns)).toMatchObject({
			date_column: 'Data contabile',
			amount_mode: 'signed',
			amount_column: 'Importo',
			description_columns: ['Descrizione'],
			balance_column: 'Saldo',
		})
	})

	it('recognises separate debit and credit columns', () => {
		const columns = ['Date', 'Payee', 'Debit', 'Credit']
		expect(guessColumns(draft, columns)).toMatchObject({
			date_column: 'Date',
			amount_mode: 'split',
			amount_column: null,
			debit_column: 'Debit',
			credit_column: 'Credit',
			description_columns: ['Payee'],
			balance_column: null,
		})
	})

	it('keeps the choices already made', () => {
		const chosen = { ...draft, date_column: 'Valuta', description_columns: ['Causale'] }
		const guessed = guessColumns(chosen, ['Data', 'Valuta', 'Causale', 'Descrizione', 'Importo'])
		expect(guessed.date_column).toBe('Valuta')
		expect(guessed.description_columns).toEqual(['Causale'])
	})
})

describe('importResultText', () => {
	it.each([
		[3, 0, '3 new Transactions.'],
		[1, 2, '1 new Transaction, 2 skipped because they were already imported.'],
		[0, 1, 'No new Transactions, 1 skipped because it was already imported.'],
	])('%i new, %i skipped', (rowsNew, rowsSkipped, text) => {
		expect(importResultText(rowsNew, rowsSkipped)).toBe(text)
	})
})
