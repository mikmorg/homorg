export function eventTypeToMessage(
	eventType: string,
	snapshotName: string,
	parentName: string | undefined
): { type: 'success' | 'context' | 'create' | 'error'; message: string } {
	let type: 'success' | 'context' | 'create' | 'error' = 'success';
	let message = '';
	switch (eventType) {
		case 'ItemCreated':
			type = 'create';
			message = parentName ? `Created: ${snapshotName || 'item'} → ${parentName}` : `Created: ${snapshotName || 'item'}`;
			break;
		case 'ItemMoved':
			message = parentName ? `Moved: ${snapshotName || 'item'} → ${parentName}` : `Moved: ${snapshotName || 'item'}`;
			break;
		case 'ItemImageAdded':
			message = `Photo added${snapshotName ? ': ' + snapshotName : ''}`;
			break;
		case 'ItemUpdated':
			message = `Updated: ${snapshotName || 'item'}`;
			break;
		case 'ItemDeleted':
			type = 'error';
			message = `Deleted: ${snapshotName || 'item'}`;
			break;
		default:
			message = eventType.replace(/([A-Z])/g, ' $1').trim();
	}
	return { type, message };
}
