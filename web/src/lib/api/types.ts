// ─── Auth ────────────────────────────────────────────────────────────────────

export interface UserPublic {
	id: string;
	username: string;
	display_name: string | null;
	role: 'admin' | 'member' | 'readonly';
	is_active: boolean;
	container_id: string | null;
	created_at: string;
}

export interface AuthResponse {
	access_token: string;
	refresh_token: string;
	expires_in: number;
	user: UserPublic;
}

export interface LoginRequest {
	username: string;
	password: string;
	device_name?: string;
}

export interface RegisterRequest {
	username: string;
	password: string;
	invite_code: string;
	display_name?: string;
}

export interface SetupRequest {
	username: string;
	password: string;
	display_name?: string;
}

export interface InviteResponse {
	code: string;
	expires_at: string;
}

// ─── Items ───────────────────────────────────────────────────────────────────

export type Condition = 'new' | 'like_new' | 'good' | 'fair' | 'poor' | 'broken';
export const CONDITIONS: Condition[] = ['new', 'like_new', 'good', 'fair', 'poor', 'broken'];

export interface ExternalCode {
	type: string; // code_type (serialized as "type")
	value: string;
}

export interface ImageEntry {
	path: string;
	caption: string | null;
	order: number;
}

export interface Item {
	id: string;
	system_barcode: string | null;
	node_id: string;
	name: string | null;
	description: string | null;
	category: string | null;
	category_id: string | null;
	tags: string[];
	is_container: boolean;
	container_path: string | null;
	parent_id: string | null;
	coordinate: unknown | null;
	location_schema: unknown | null;
	max_capacity_cc: string | null; // Decimal comes as string
	max_weight_grams: string | null;
	container_type_id: string | null;
	dimensions: unknown | null;
	weight_grams: string | null;
	is_fungible: boolean;
	fungible_quantity: number | null;
	fungible_unit: string | null;
	external_codes: ExternalCode[];
	condition: Condition | null;
	acquisition_date: string | null;
	acquisition_cost: string | null;
	current_value: string | null;
	depreciation_rate: string | null;
	warranty_expiry: string | null;
	currency: string | null;
	metadata: Record<string, unknown>;
	images: ImageEntry[];
	is_deleted: boolean;
	deleted_at: string | null;
	created_at: string;
	updated_at: string;
	created_by: string | null;
	updated_by: string | null;
	classification_confidence: number | null;
	needs_review: boolean;
	ai_description: string | null;
	ancestors?: AncestorEntry[];
}

export interface ItemSummary {
	id: string;
	system_barcode: string | null;
	name: string | null;
	category: string | null;
	is_container: boolean;
	container_path: string | null;
	parent_id: string | null;
	condition: Condition | null;
	tags: string[];
	is_deleted: boolean;
	created_at: string;
	updated_at: string;
}

export interface AncestorEntry {
	id: string;
	system_barcode: string | null;
	name: string | null;
	node_id: string;
	depth: number;
}

export interface ContainerStats {
	child_count: number;
	descendant_count: number;
	total_weight_grams: number | null;
	capacity_used_cc: number | null;
	max_capacity_cc: number | null;
	utilization_pct: number | null;
}

export interface CreateItemRequest {
	parent_id: string;
	name?: string;
	description?: string;
	system_barcode?: string;
	category?: string;
	tags?: string[];
	is_container?: boolean;
	is_fungible?: boolean;
	fungible_quantity?: number;
	fungible_unit?: string;
	coordinate?: unknown;
	location_schema?: unknown;
	condition?: Condition;
	dimensions?: unknown;
	weight_grams?: number;
	acquisition_date?: string;
	acquisition_cost?: number;
	current_value?: number;
	depreciation_rate?: number;
	warranty_expiry?: string;
	currency?: string;
	metadata?: Record<string, unknown>;
	external_codes?: ExternalCode[];
	container_type_id?: string;
	max_capacity_cc?: number;
	max_weight_grams?: number;
}

export interface UpdateItemRequest {
	name?: string;
	description?: string;
	category?: string;
	tags?: string[];
	is_container?: boolean;
	is_fungible?: boolean;
	fungible_unit?: string | null;
	coordinate?: unknown;
	condition?: Condition | null;
	dimensions?: unknown;
	weight_grams?: number | null;
	acquisition_date?: string | null;
	acquisition_cost?: number | null;
	current_value?: number | null;
	depreciation_rate?: number | null;
	warranty_expiry?: string | null;
	currency?: string | null;
	metadata?: Record<string, unknown>;
	container_type_id?: string | null;
	max_capacity_cc?: number | null;
	max_weight_grams?: number | null;
}

export interface MoveItemRequest {
	container_id: string;
	coordinate?: unknown;
}

export interface AdjustQuantityRequest {
	new_quantity: number;
	reason?: string;
}

export interface AssignBarcodeRequest {
	barcode: string;
}

// ─── Barcodes ────────────────────────────────────────────────────────────────

export type BarcodeResolution =
	| { type: 'system'; barcode: string; item_id: string }
	| { type: 'external'; code_type: string; value: string; item_ids: string[] }
	| { type: 'unknown_system'; barcode: string }
	| { type: 'unknown'; value: string };

export interface GeneratedBarcode {
	barcode: string;
}

// ─── Stocker ─────────────────────────────────────────────────────────────────

export interface ScanSession {
	id: string;
	user_id: string;
	active_container_id: string | null;
	started_at: string;
	ended_at: string | null;
	items_scanned: number;
	items_created: number;
	items_moved: number;
	items_errored: number;
	device_id: string | null;
	notes: string | null;
}

export interface StartSessionRequest {
	device_id?: string;
	notes?: string;
	initial_container_barcode?: string;
}

export type StockerBatchEvent =
	| { type: 'set_context'; barcode: string; scanned_at: string }
	| { type: 'move_item'; barcode: string; coordinate?: unknown; scanned_at: string }
	| {
			type: 'create_and_place';
			barcode: string;
			name?: string;
			description?: string;
			category?: string;
			category_id?: string;
			tags?: string[];
			is_container?: boolean;
			coordinate?: unknown;
			condition?: Condition;
			metadata?: Record<string, unknown>;
			scanned_at: string;
			is_fungible?: boolean;
			fungible_quantity?: number;
			fungible_unit?: string;
			external_codes?: ExternalCode[];
			container_type_id?: string;
		}
	| { type: 'resolve'; barcode: string; scanned_at: string };

export interface StockerBatchRequest {
	events: StockerBatchEvent[];
}

export type StockerBatchResult =
	| { type: 'context_set'; index: number; status: string; context_set: string }
	| { type: 'moved'; index: number; status: string; event_id: string }
	| { type: 'created'; index: number; status: string; event_id: string; item_id: string; needs_details: boolean }
	| { type: 'resolved'; index: number; status: string; resolution: BarcodeResolution };

export interface StockerBatchError {
	index: number;
	code: string;
	message: string;
}

export interface StockerBatchResponse {
	processed: number;
	results: StockerBatchResult[];
	errors: StockerBatchError[];
}

// ─── Containers ──────────────────────────────────────────────────────────────

export interface ChildrenParams {
	cursor?: string;
	limit?: number;
	sort_by?: 'name' | 'created_at' | 'updated_at' | 'category' | 'system_barcode';
	sort_dir?: 'asc' | 'desc';
}

export interface DescendantsParams {
	max_depth?: number;
	limit?: number;
}

// ─── Search ──────────────────────────────────────────────────────────────────

export interface SearchParams {
	q?: string;
	path?: string;
	category?: string;
	condition?: Condition;
	container_id?: string;
	tags?: string;
	is_container?: boolean;
	min_value?: number;
	max_value?: number;
	cursor?: string;
	limit?: number;
}

// ─── Taxonomy ────────────────────────────────────────────────────────────────

export interface Category {
	id: string;
	name: string;
	description: string | null;
	parent_category_id: string | null;
	item_count?: number;
	created_at: string;
	updated_at: string;
}

export interface Tag {
	id: string;
	name: string;
	item_count?: number;
	created_at: string;
}

// ─── Container Types ─────────────────────────────────────────────────────────

export interface ContainerType {
	id: string;
	name: string;
	description: string | null;
	default_max_capacity_cc: string | null;
	default_max_weight_grams: string | null;
	default_dimensions: unknown | null;
	default_location_schema: unknown | null;
	icon: string | null;
	purpose: string | null;
	created_by: string | null;
	created_at: string;
	updated_at: string;
}

// ─── Events ──────────────────────────────────────────────────────────────────

// ─── Location Schema & Coordinates ──────────────────────────────────────────

export interface AbstractLocationSchema {
	type: 'abstract';
	labels?: string[];
}

export interface GridLocationSchema {
	type: 'grid';
	rows: number;
	columns: number;
	row_labels?: string[];
	column_labels?: string[];
}

export interface GeoLocationSchema {
	type: 'geo';
}

export type KnownLocationSchema =
	| AbstractLocationSchema
	| GridLocationSchema
	| GeoLocationSchema;

export interface AbstractCoordinate {
	type: 'abstract';
	value: string;
}

export interface GridCoordinate {
	type: 'grid';
	row: number;
	column: number;
}

export interface GeoCoordinate {
	type: 'geo';
	latitude: number;
	longitude: number;
}

export type KnownCoordinate =
	| AbstractCoordinate
	| GridCoordinate
	| GeoCoordinate;

// ─── Events ──────────────────────────────────────────────────────────────────

export interface StoredEvent {
	id: number;
	event_id: string;
	aggregate_id: string;
	aggregate_type: string;
	event_type: string;
	event_data: unknown;
	metadata: {
		correlation_id?: string;
		causation_id?: string;
		session_id?: string;
		batch_id?: string;
		scanned_at?: string;
	};
	actor_id: string | null;
	created_at: string;
	sequence_number: number;
	schema_version: number;
}

// ─── System ──────────────────────────────────────────────────────────────────

export interface HealthResponse {
	status: string;
	database: string;
	version: string;
	setup_required?: boolean;
}

export interface CategoryCount {
	category: string | null;
	count: number;
}

export interface ConditionCount {
	condition: string | null;
	count: number;
}

export interface StatsResponse {
	total_items: number;
	total_containers: number;
	total_events: number;
	total_users: number;
	items_by_category: CategoryCount[];
	items_by_condition: ConditionCount[];
}

// ─── Users ───────────────────────────────────────────────────────────────────

export interface UpdateUserRequest {
	display_name?: string;
	password?: string;
	current_password?: string;
}

export interface UpdateRoleRequest {
	role: 'admin' | 'member' | 'readonly';
}

// ─── API Error ───────────────────────────────────────────────────────────────

export interface ApiError {
	status: number;
	message: string;
	code?: string;
}
