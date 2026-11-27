import { invoke } from '@tauri-apps/api/core'

export type CategoryKind = 'income' | 'expense'

export type NewCategory = {
	name: string
	kind: CategoryKind
	/** Create it as a child of this top-level Category. */
	parent_id: string | null
}

export type Category = {
	id: string
	name: string
	kind: CategoryKind
	parent_id: string | null
	color: string | null
	/** Display order among siblings. */
	position: number
}

export const categoriesApi = {
	list: () => invoke<Category[]>('categories_list'),
	create: (category: NewCategory) => invoke<Category>('category_create', { category }),
	rename: (id: string, name: string) => invoke<Category>('category_rename', { id, name }),
	/** Move to `index` among its siblings. */
	move: (id: string, index: number) => invoke<Category>('category_move', { id, index }),
	delete: (id: string) => invoke<void>('category_delete', { id }),
}
