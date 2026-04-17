import { describe, it, expect, beforeEach, vi } from 'vitest';

/**
 * Mock of the API to simulate ancestors response
 */
function createMockApi() {
	return {
		containers: {
			children: vi.fn(async () => []),
			ancestors: vi.fn(async (id: string) => {
				// Simulate the backend behavior:
				// ancestors() returns ancestors only (excludes current item)
				if (id === 'level3-id') {
					return [
						{ id: 'root-id', name: 'Root', system_barcode: null, node_id: '1', depth: 0 },
						{ id: 'level1-id', name: 'Level 1', system_barcode: null, node_id: '2', depth: 1 },
						{ id: 'level2-id', name: 'Level 2', system_barcode: null, node_id: '3', depth: 2 }
						// Note: level3 is NOT included (current item)
					];
				}
				return [];
			})
		},
		items: {
			get: vi.fn(async (id: string) => {
				if (id === 'level3-id') {
					return { id: 'level3-id', name: 'Level 3', is_container: true };
				}
				return null;
			})
		}
	};
}

describe('Browse breadcrumb', () => {
	it('should include current item in breadcrumb display', async () => {
		const api = createMockApi();
		const ROOT_ID = 'root-id';

		// Simulate loading level3
		const id = 'level3-id';
		const res = await api.containers.children(id, { limit: 51 });
		expect(res).toEqual([]);

		const ancs = await api.containers.ancestors(id);
		const item = await api.items.get(id);

		// Build breadcrumb as the current code does
		let breadcrumb = ancs.map((a) => ({ id: a.id, name: a.name ?? 'Container' }));

		console.log('Ancestors returned:', ancs);
		console.log('Current item:', item);
		console.log('Breadcrumb array:', breadcrumb);

		// BUG: breadcrumb is [Root, Level1, Level2] but should include Level3!
		// The current code shows breadcrumb[breadcrumb.length - 1] as the current item,
		// but that's actually Level2 (the parent), not Level3 (the current item)

		expect(breadcrumb).toHaveLength(3); // [Root, Level1, Level2]

		// This demonstrates the bug: the last item in breadcrumb is NOT the current item
		const headerText = breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].name : 'Container';
		expect(headerText).toBe('Level 2'); // BUG: shows parent, not current item!
		expect(headerText).not.toBe('Level 3'); // Should show current but doesn't

		// The breadcrumb displayed is also incomplete
		const displayedBreadcrumb = breadcrumb.slice(1); // Skip root
		expect(displayedBreadcrumb).toHaveLength(2); // [Level1, Level2]
		const lastDisplayed = displayedBreadcrumb[displayedBreadcrumb.length - 1]?.name;
		expect(lastDisplayed).toBe('Level 2'); // Missing Level 3!
		expect(lastDisplayed).not.toBe('Level 3');
	});

	it('should make correct items clickable in breadcrumb', () => {
		// Current implementation: breadcrumb.slice(1) shows all except root
		// Then {#if i < breadcrumb.length - 2} determines if link or text

		const breadcrumb = [
			{ id: 'root-id', name: 'Root' },
			{ id: 'level1-id', name: 'Level 1' },
			{ id: 'level2-id', name: 'Level 2' }
			// Level 3 is missing!
		];

		const displayItems = breadcrumb.slice(1);

		// Simulate the rendering logic
		const links = displayItems
			.map((crumb, i) => ({
				name: crumb.name,
				isLink: i < breadcrumb.length - 2
			}));

		console.log('Breadcrumb click logic:', links);

		// Expected: Level1 should be link, Level2 should be text (current)
		expect(links[0].isLink).toBe(true);  // Level1 is a link
		expect(links[1].isLink).toBe(false); // Level2 is text

		// But the actual current item (Level3) is missing entirely!
		// So Level2 (the parent) gets shown as the current item
	});

	it('should fix breadcrumb to include current item', () => {
		// Fix: append current item to breadcrumb
		const ancs = [
			{ id: 'root-id', name: 'Root' },
			{ id: 'level1-id', name: 'Level 1' },
			{ id: 'level2-id', name: 'Level 2' }
		];
		const currentItem = { id: 'level3-id', name: 'Level 3' };

		// FIXED breadcrumb should include current item
		const breadcrumbFixed = [
			...ancs.map((a) => ({ id: a.id, name: a.name ?? 'Container' })),
			{ id: currentItem.id, name: currentItem.name ?? 'Container' }
		];

		console.log('Fixed breadcrumb:', breadcrumbFixed);
		expect(breadcrumbFixed).toHaveLength(4); // [Root, Level1, Level2, Level3]
		expect(breadcrumbFixed[breadcrumbFixed.length - 1].name).toBe('Level 3');

		// With fixed breadcrumb, the rendering logic needs adjustment too
		const displayItems = breadcrumbFixed.slice(1); // [Level1, Level2, Level3]

		const links = displayItems.map((crumb, i) => ({
			name: crumb.name,
			// Fix the index calculation: now we check against the slice length, not original
			isLink: i < displayItems.length - 1
		}));

		console.log('Fixed click logic:', links);
		expect(links[0].isLink).toBe(true);  // Level1 is a link
		expect(links[1].isLink).toBe(true);  // Level2 is a link
		expect(links[2].isLink).toBe(false); // Level3 is text (current)
	});
});
