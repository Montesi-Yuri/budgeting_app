import { useCallback, useEffect, useState } from 'react'
import { Category, CategoryKind, errorMessage, categoriesApi } from '@/shared/api'
import { useAction } from '@/shared/hooks'
import { KIND_LABEL, siblingPlace } from '../lib/categories'

type Adding = { parent: Category | null } | null

export default function CategoriesPage() {
	const [categories, setCategories] = useState<Category[] | null>(null)
	const [adding, setAdding] = useState<Adding>(null)
	const [renaming, setRenaming] = useState<string | null>(null)
	const [error, setError] = useState<string | null>(null)

	const reload = useCallback(() => {
		categoriesApi.list().then(setCategories, (e) => setError(errorMessage(e)))
	}, [])
	useEffect(reload, [reload])

	const run = useAction(reload, setError)

	function remove(c: Category) {
		const hasChildren = categories?.some((x) => x.parent_id === c.id)
		const msg = hasChildren
			? `Delete the Category "${c.name}" and its children? Their Transactions become Uncategorised.`
			: `Delete the Category "${c.name}"? Its Transactions become Uncategorised.`
		if (window.confirm(msg)) run(() => categoriesApi.delete(c.id))
	}

	return (
		<>
			<div className="heading">
				<h1>Categories</h1>
				{adding === null && (
					<button className="primary" onClick={() => setAdding({ parent: null })}>
						Add Category
					</button>
				)}
			</div>
			<p className="muted small">
				A Transaction in an income Category is Income; in an expense Category it is an Expense, and a positive
				amount there is a Refund. Categories have two levels: a parent and its children.
			</p>
			{error && <p className="error">{error}</p>}

			{adding && (
				<NewCategoryForm
					parent={adding.parent}
					onSubmit={async (name, kind) => {
						const ok = await run(() =>
							categoriesApi.create({ name, kind, parent_id: adding.parent?.id ?? null }))
						if (ok) setAdding(null)
					}}
					onCancel={() => setAdding(null)}
				/>
			)}

			{categories && categories.length === 0 && !adding && (
				<p className="muted">No Categories yet. Add expense Categories such as Groceries and income ones such as Salary.</p>
			)}

			{categories && categories.length > 0 && (
				<ul className="list">
					{categories.map((c) => {
						const { index, count } = siblingPlace(categories, c.id)
						return (
							<li key={c.id} className={c.parent_id ? 'row-item child' : 'row-item'}>
								{renaming === c.id
									? (
											<RenameForm
												initial={c.name}
												onSubmit={async (name) => {
													if (await run(() => categoriesApi.rename(c.id, name))) setRenaming(null)
												}}
												onCancel={() => setRenaming(null)}
											/>
										)
									: (
											<span className="grow">
												{c.name}
												{' '}
												{!c.parent_id && <span className={`badge kind-${c.kind}`}>{KIND_LABEL[c.kind]}</span>}
											</span>
										)}
								<span className="actions">
									<button title="Move up" disabled={index <= 0} onClick={() => run(() => categoriesApi.move(c.id, index - 1))}>
										↑
									</button>
									<button
										title="Move down"
										disabled={index >= count - 1}
										onClick={() => run(() => categoriesApi.move(c.id, index + 1))}
									>
										↓
									</button>
									{!c.parent_id && <button onClick={() => setAdding({ parent: c })}>Add child</button>}
									<button onClick={() => setRenaming(c.id)}>Rename</button>
									<button className="danger" onClick={() => remove(c)}>
										Delete
									</button>
								</span>
							</li>
						)
					})}
				</ul>
			)}
		</>
	)
}

function NewCategoryForm({
	parent,
	onSubmit,
	onCancel,
}: {
	parent: Category | null
	onSubmit: (name: string, kind: CategoryKind) => Promise<void>
	onCancel: () => void
}) {
	const [name, setName] = useState('')
	const [kind, setKind] = useState<CategoryKind>(parent?.kind ?? 'expense')
	return (
		<form
			className="card form"
			onSubmit={(e) => {
				e.preventDefault()
				onSubmit(name, kind)
			}}
		>
			<h2>{parent ? `New child of ${parent.name}` : 'New Category'}</h2>
			<label className="field">
				<span>Name</span>
				<input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Groceries" autoFocus />
			</label>
			{parent
				? (
						<p className="muted small">
							A child has the same kind as its parent:
							{KIND_LABEL[parent.kind].toLowerCase()}
							.
						</p>
					)
				: (
						<label className="field">
							<span>Kind</span>
							<select value={kind} onChange={(e) => setKind(e.target.value as CategoryKind)}>
								<option value="expense">Expense</option>
								<option value="income">Income</option>
							</select>
						</label>
					)}
			<div className="actions">
				<button type="submit" className="primary">
					Add Category
				</button>
				<button type="button" onClick={onCancel}>
					Cancel
				</button>
			</div>
		</form>
	)
}

function RenameForm({
	initial,
	onSubmit,
	onCancel,
}: {
	initial: string
	onSubmit: (name: string) => Promise<void>
	onCancel: () => void
}) {
	const [name, setName] = useState(initial)
	return (
		<form
			className="grow inline"
			onSubmit={(e) => {
				e.preventDefault()
				onSubmit(name)
			}}
		>
			<input value={name} onChange={(e) => setName(e.target.value)} autoFocus aria-label="Category name" />
			<button type="submit" className="primary">
				Save
			</button>
			<button type="button" onClick={onCancel}>
				Cancel
			</button>
		</form>
	)
}
