#!/usr/bin/env python3
"""Seed a running Homorg instance with realistic household inventory data.

Usage:
    python3 scripts/seed.py [--base-url http://localhost:8080]

Requires: pip install requests  (usually pre-installed)
"""

import argparse
import io
import struct
import sys
import time
from typing import Any, Optional

import requests

ROOT_ID = "00000000-0000-0000-0000-000000000001"
ADMIN_USER = "admin"
ADMIN_PASS = "seedpassword123"

# Counters
_counts = {"containers": 0, "items": 0, "images": 0, "fungible": 0}


# ─── Minimal JPEG generator ─────────────────────────────────────────────────
# Creates a valid 8×8 single-color JPEG (~600 bytes) using raw JFIF structure.
# No PIL/Pillow needed.

def _make_jpeg(r: int, g: int, b: int) -> bytes:
    """Generate a minimal valid JPEG file with a solid color."""
    # Use a BMP instead — simpler to construct and the backend accepts image/bmp
    # Actually the backend checks magic bytes and only allows jpeg/png/webp/gif.
    # Let's generate a minimal valid PNG instead (easier than JPEG to construct).
    return _make_png(r, g, b)


def _make_png(r: int, g: int, b: int, size: int = 32) -> bytes:
    """Generate a minimal valid PNG file with a solid color (size×size pixels)."""
    import zlib

    def _chunk(chunk_type: bytes, data: bytes) -> bytes:
        c = chunk_type + data
        crc = struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)
        return struct.pack(">I", len(data)) + c + crc

    # PNG signature
    sig = b"\x89PNG\r\n\x1a\n"

    # IHDR: width, height, bit_depth=8, color_type=2 (RGB), compression=0, filter=0, interlace=0
    ihdr_data = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    ihdr = _chunk(b"IHDR", ihdr_data)

    # IDAT: raw image data (each row: filter byte 0 + RGB pixels)
    raw_row = b"\x00" + bytes([r, g, b]) * size
    raw_data = raw_row * size
    compressed = zlib.compress(raw_data)
    idat = _chunk(b"IDAT", compressed)

    # IEND
    iend = _chunk(b"IEND", b"")

    return sig + ihdr + idat + iend


# ─── Color palette for images ───────────────────────────────────────────────

IMAGE_COLORS = [
    (70, 130, 180),   # steel blue
    (60, 179, 113),   # medium sea green
    (205, 92, 92),    # indian red
    (218, 165, 32),   # goldenrod
    (147, 112, 219),  # medium purple
    (255, 160, 122),  # light salmon
    (0, 128, 128),    # teal
    (210, 105, 30),   # chocolate
    (100, 149, 237),  # cornflower blue
    (144, 238, 144),  # light green
]


# ─── API client ──────────────────────────────────────────────────────────────

class Api:
    def __init__(self, base_url: str):
        self.base = base_url.rstrip("/") + "/api/v1"
        self.files_base = base_url.rstrip("/")
        self.token: Optional[str] = None
        self.s = requests.Session()

    def _headers(self) -> dict:
        h = {}
        if self.token:
            h["Authorization"] = f"Bearer {self.token}"
        return h

    def _retry(self, method: str, url: str, max_retries: int = 5, **kwargs) -> requests.Response:
        """Execute an HTTP request with automatic 429 retry + backoff."""
        for attempt in range(max_retries):
            r = self.s.request(method, url, headers=self._headers(), **kwargs)
            if r.status_code != 429:
                return r
            # Parse wait time from "Wait for Ns" or default to exponential backoff
            wait = 2 ** attempt
            body = r.text
            if "Wait for" in body:
                try:
                    wait = int(body.split("Wait for")[1].strip().rstrip("s "))
                except (ValueError, IndexError):
                    pass
            wait = min(wait + 1, 120)
            print(f"    [rate-limited, waiting {wait}s...]")
            time.sleep(wait)
        return r  # return last response even if still 429

    def _post(self, path: str, json: Any = None, files: Any = None, data: Any = None) -> requests.Response:
        return self._retry("POST", f"{self.base}{path}", json=json, files=files, data=data)

    def _put(self, path: str, json: Any = None) -> requests.Response:
        return self._retry("PUT", f"{self.base}{path}", json=json)

    def _get(self, path: str, params: Any = None) -> requests.Response:
        return self._retry("GET", f"{self.base}{path}", params=params)

    def authenticate(self):
        """Try setup first, fall back to login."""
        username = getattr(self, "username", ADMIN_USER)
        password = getattr(self, "password", ADMIN_PASS)
        body = {"username": username, "password": password}
        r = self._post("/auth/setup", json=body)
        if r.status_code in (200, 201):
            self.token = r.json()["access_token"]
            print(f"  Created admin user '{username}'")
            return
        if r.status_code == 409:
            print("  Setup already done, logging in...")
        # Setup already done — try login
        r = self._post("/auth/login", json=body)
        if r.status_code == 200:
            self.token = r.json()["access_token"]
            print(f"  Logged in as '{username}'")
            return
        print(f"  Login failed ({r.status_code}). Check --username and --password.", file=sys.stderr)
        sys.exit(1)

    def create_container_type(self, name: str, **kwargs) -> dict:
        body: dict[str, Any] = {"name": name}
        body.update(kwargs)
        r = self._post("/container-types", json=body)
        if r.status_code not in (200, 201):
            # May already exist
            if "duplicate" in r.text.lower() or "conflict" in r.text.lower():
                return {"id": None, "name": name}
            print(f"  WARN: container-type '{name}': {r.status_code} {r.text}")
            return {"id": None, "name": name}
        return r.json()

    def create_category(self, name: str, description: str = "") -> dict:
        r = self._post("/categories", json={"name": name, "description": description})
        if r.status_code not in (200, 201):
            return {"id": None, "name": name}
        return r.json()

    def create_tag(self, name: str) -> dict:
        r = self._post("/tags", json={"name": name})
        if r.status_code not in (200, 201):
            return {"id": None, "name": name}
        return r.json()

    def create_item(self, parent_id: str, name: str, **kwargs) -> dict:
        body: dict[str, Any] = {"parent_id": parent_id, "name": name}
        body.update(kwargs)
        is_container = body.get("is_container", False)
        is_fungible = body.get("is_fungible", False)
        r = self._post("/items", json=body)
        if r.status_code not in (200, 201):
            print(f"  ERROR creating '{name}': {r.status_code} {r.text}", file=sys.stderr)
            return {}
        data = r.json()
        item_id = data.get("aggregate_id") or data.get("id", "")
        if is_container:
            _counts["containers"] += 1
        elif is_fungible:
            _counts["fungible"] += 1
        else:
            _counts["items"] += 1
        return data

    def get_item(self, item_id: str) -> dict:
        r = self._get(f"/items/{item_id}")
        if r.status_code != 200:
            return {}
        return r.json()

    def upload_image(self, item_id: str, png_data: bytes, caption: str, order: int = 0):
        if not item_id:
            return
        url = f"{self.base}/items/{item_id}/images"
        for attempt in range(5):
            files = {"file": ("image.png", io.BytesIO(png_data), "image/png")}
            form = {"caption": caption, "order": str(order)}
            r = self.s.post(url, files=files, data=form, headers=self._headers())
            if r.status_code in (200, 201):
                _counts["images"] += 1
                return
            if r.status_code != 429:
                print(f"  WARN: image upload for {item_id}: {r.status_code} {r.text}")
                return
            wait = 2 ** attempt
            body = r.text
            if "Wait for" in body:
                try:
                    wait = int(body.split("Wait for")[1].strip().rstrip("s "))
                except (ValueError, IndexError):
                    pass
            wait = min(wait + 1, 120)
            print(f"    [rate-limited, waiting {wait}s...]")
            time.sleep(wait)

    def item_id_from_event(self, event: dict) -> str:
        return event.get("aggregate_id", "")


# ─── Helpers ─────────────────────────────────────────────────────────────────

def abstract_schema(labels: list[str]) -> dict:
    return {"type": "abstract", "labels": labels}


def grid_schema(rows: int, cols: int, row_labels: list[str] = None, col_labels: list[str] = None) -> dict:
    s: dict[str, Any] = {"type": "grid", "rows": rows, "columns": cols}
    if row_labels:
        s["row_labels"] = row_labels
    if col_labels:
        s["column_labels"] = col_labels
    return s


def geo_coord(lat: float, lon: float) -> dict:
    return {"type": "geo", "latitude": lat, "longitude": lon}


def abstract_coord(value: str) -> dict:
    return {"type": "abstract", "value": value}


def grid_coord(row: int, col: int) -> dict:
    return {"type": "grid", "row": row, "column": col}


def dims(w: float, h: float, d: float) -> dict:
    return {"width_cm": w, "height_cm": h, "depth_cm": d}


_color_idx = 0
def next_image(caption: str = "") -> tuple[bytes, str]:
    global _color_idx
    r, g, b = IMAGE_COLORS[_color_idx % len(IMAGE_COLORS)]
    _color_idx += 1
    return _make_png(r, g, b), caption


# ─── Seed data ───────────────────────────────────────────────────────────────

def seed(api: Api):
    print("Authenticating...")
    api.authenticate()

    # ── Container types ──────────────────────────────────────────────────
    print("Creating container types...")
    ct_types = {}
    for name, icon, purpose in [
        ("House", "house", "storage"),
        ("Room", "room", "storage"),
        ("Shelf", "shelf", "storage"),
        ("Bin", "bin", "storage"),
        ("Box", "box", "storage"),
        ("Tote", "bin", "storage"),
        ("Bucket", "bin", "storage"),
        ("Crate", "bin", "storage"),
        ("Chest", "bin", "storage"),
        ("Case", "bin", "storage"),
        ("Drawer", "drawer", "storage"),
        ("Cabinet", "shelf", "storage"),
        ("Rack", "shelf", "storage"),
        ("Kit", "bin", "storage"),
    ]:
        ct = api.create_container_type(name, icon=icon, purpose=purpose)
        ct_types[name] = ct.get("id")

    # ── Categories ───────────────────────────────────────────────────────
    print("Creating categories...")
    for name in [
        "Electronics", "Books", "Clothing", "Tools", "Kitchen",
        "Cleaning", "Health", "Toys", "Sports", "Seasonal",
        "Documents", "Crafts", "Garden", "Emergency", "Food",
        "Furniture", "Office", "Personal", "Media", "Automotive",
    ]:
        api.create_category(name)

    # ── Tags ─────────────────────────────────────────────────────────────
    print("Creating tags...")
    for name in [
        "fragile", "heavy", "valuable", "seasonal", "vintage",
        "needs-repair", "warranty", "consumable", "dangerous",
        "sentimental", "outdoor", "hand-tool", "power-tool",
    ]:
        api.create_tag(name)

    # ── House ────────────────────────────────────────────────────────────
    print("Creating house...")
    house_ev = api.create_item(
        ROOT_ID, "742 Evergreen Terrace",
        is_container=True,
        description="A two-story family home in a quiet suburban neighborhood. Full basement and attached two-car garage.",
        coordinate=geo_coord(41.8781, -87.6298),
        container_type_id=ct_types.get("House"),
        dimensions=dims(1500, 300, 1200),
    )
    house_id = api.item_id_from_event(house_ev)
    if not house_id:
        print("FATAL: could not create house", file=sys.stderr)
        sys.exit(1)

    img, cap = next_image("Front view of 742 Evergreen Terrace")
    api.upload_image(house_id, img, cap, 0)

    # ── Living Room ──────────────────────────────────────────────────────
    print("  Living Room...")
    lr_ev = api.create_item(
        house_id, "Living Room",
        is_container=True,
        description="Main living area with hardwood floors, bay window, and built-in bookshelf.",
        location_schema=abstract_schema(["TV Stand", "Bookshelf", "Coffee Table"]),
        container_type_id=ct_types.get("Room"),
        dimensions=dims(500, 270, 400),
    )
    lr_id = api.item_id_from_event(lr_ev)

    img, cap = next_image("Living room overview")
    api.upload_image(lr_id, img, cap)

    # TV Stand items
    api.create_item(lr_id, "HDMI Cables", coordinate=abstract_coord("TV Stand"),
                    category="Electronics", tags=["fragile"], is_fungible=True,
                    fungible_quantity=4, fungible_unit="pcs",
                    external_codes=[{"type": "UPC", "value": "012345678901"}],
                    condition="good", acquisition_cost=7.99, currency="USD")
    api.create_item(lr_id, "TV Remote Control", coordinate=abstract_coord("TV Stand"),
                    category="Electronics", condition="good")
    ev = api.create_item(lr_id, "Roku Streaming Box", coordinate=abstract_coord("TV Stand"),
                    category="Electronics", condition="good", acquisition_cost=39.99,
                    currency="USD", warranty_expiry="2027-06-15", tags=["warranty"])
    api.upload_image(api.item_id_from_event(ev), *next_image("Roku player under TV"))
    api.create_item(lr_id, "Soundbar", coordinate=abstract_coord("TV Stand"),
                    category="Electronics", condition="good", acquisition_cost=149.99,
                    currency="USD", weight_grams=3200, dimensions=dims(90, 7, 10),
                    tags=["valuable"])

    # Bookshelf items
    books = [
        "To Kill a Mockingbird", "1984", "The Great Gatsby", "Pride and Prejudice",
        "The Catcher in the Rye", "Brave New World", "Lord of the Flies",
        "Animal Farm", "The Hobbit", "Fahrenheit 451", "Dune", "Slaughterhouse-Five",
    ]
    for b in books:
        api.create_item(lr_id, b, coordinate=abstract_coord("Bookshelf"),
                        category="Books", condition="good")
    for game in ["Settlers of Catan", "Ticket to Ride", "Pandemic"]:
        api.create_item(lr_id, game, coordinate=abstract_coord("Bookshelf"),
                        category="Media", condition="good", tags=["sentimental"])

    # Coffee Table items
    api.create_item(lr_id, "Cork Coasters", coordinate=abstract_coord("Coffee Table"),
                    is_fungible=True, fungible_quantity=6, fungible_unit="pcs",
                    category="Furniture", condition="fair")
    api.create_item(lr_id, "National Geographic Magazine", coordinate=abstract_coord("Coffee Table"),
                    category="Books", condition="good")
    api.create_item(lr_id, "Vanilla Candle", coordinate=abstract_coord("Coffee Table"),
                    category="Personal", condition="new")

    # ── Kitchen ──────────────────────────────────────────────────────────
    print("  Kitchen...")
    kit_ev = api.create_item(
        house_id, "Kitchen",
        is_container=True,
        description="Galley kitchen with tile backsplash, gas range, and breakfast nook.",
        location_schema=abstract_schema(["Pantry", "Under Sink", "Countertop", "Junk Drawer"]),
        container_type_id=ct_types.get("Room"),
    )
    kit_id = api.item_id_from_event(kit_ev)
    api.upload_image(kit_id, *next_image("Kitchen countertop and appliances"))

    # Pantry
    api.create_item(kit_id, "Canned Black Beans", coordinate=abstract_coord("Pantry"),
                    category="Food", is_fungible=True, fungible_quantity=12, fungible_unit="cans",
                    condition="new", tags=["consumable"])
    api.create_item(kit_id, "Dry Pasta", coordinate=abstract_coord("Pantry"),
                    category="Food", is_fungible=True, fungible_quantity=6, fungible_unit="boxes",
                    condition="new", tags=["consumable"])
    api.create_item(kit_id, "Spice Rack Set", coordinate=abstract_coord("Pantry"),
                    category="Kitchen", condition="good",
                    external_codes=[{"type": "UPC", "value": "098765432109"}])
    api.create_item(kit_id, "Jasmine Rice 5lb Bag", coordinate=abstract_coord("Pantry"),
                    category="Food", condition="new", weight_grams=2268, tags=["consumable"])
    api.create_item(kit_id, "All-Purpose Flour", coordinate=abstract_coord("Pantry"),
                    category="Food", condition="new", weight_grams=2268, tags=["consumable"])

    # Under Sink
    api.create_item(kit_id, "Dish Soap", coordinate=abstract_coord("Under Sink"),
                    category="Cleaning", condition="good", tags=["consumable"])
    api.create_item(kit_id, "Kitchen Sponges", coordinate=abstract_coord("Under Sink"),
                    category="Cleaning", is_fungible=True, fungible_quantity=10, fungible_unit="pcs",
                    tags=["consumable"])
    api.create_item(kit_id, "Trash Bags 13gal", coordinate=abstract_coord("Under Sink"),
                    category="Cleaning", is_fungible=True, fungible_quantity=30, fungible_unit="bags",
                    tags=["consumable"])
    api.create_item(kit_id, "All-Purpose Spray Cleaner", coordinate=abstract_coord("Under Sink"),
                    category="Cleaning", condition="good", tags=["consumable", "dangerous"])

    # Countertop
    ev = api.create_item(kit_id, "Toaster 2-Slice", coordinate=abstract_coord("Countertop"),
                    category="Kitchen", condition="good", acquisition_cost=29.99, currency="USD",
                    external_codes=[{"type": "UPC", "value": "111222333444"}],
                    dimensions=dims(28, 18, 17), weight_grams=1500)
    api.upload_image(api.item_id_from_event(ev), *next_image("Silver 2-slice toaster"))
    api.create_item(kit_id, "Drip Coffee Maker", coordinate=abstract_coord("Countertop"),
                    category="Kitchen", condition="good", acquisition_cost=49.99, currency="USD",
                    tags=["warranty"])
    api.create_item(kit_id, "Knife Block with Knives", coordinate=abstract_coord("Countertop"),
                    category="Kitchen", condition="good", tags=["dangerous"],
                    acquisition_cost=79.99, currency="USD")
    api.create_item(kit_id, "Bamboo Cutting Boards", coordinate=abstract_coord("Countertop"),
                    category="Kitchen", is_fungible=True, fungible_quantity=3, fungible_unit="pcs",
                    condition="fair")

    # Junk Drawer
    api.create_item(kit_id, "AA Batteries", coordinate=abstract_coord("Junk Drawer"),
                    category="Electronics", is_fungible=True, fungible_quantity=20, fungible_unit="pcs",
                    tags=["consumable", "dangerous"])
    api.create_item(kit_id, "Duct Tape", coordinate=abstract_coord("Junk Drawer"),
                    category="Tools", condition="good")
    api.create_item(kit_id, "Kitchen Scissors", coordinate=abstract_coord("Junk Drawer"),
                    category="Kitchen", condition="good")
    api.create_item(kit_id, "Ballpoint Pens", coordinate=abstract_coord("Junk Drawer"),
                    is_fungible=True, fungible_quantity=10, fungible_unit="pcs",
                    category="Office", tags=["consumable"])
    api.create_item(kit_id, "LED Flashlight", coordinate=abstract_coord("Junk Drawer"),
                    category="Emergency", condition="good")

    # ── Master Bedroom ───────────────────────────────────────────────────
    print("  Master Bedroom...")
    mb_ev = api.create_item(
        house_id, "Master Bedroom",
        is_container=True,
        description="Spacious primary bedroom with walk-in closet and ensuite bath access.",
        location_schema=abstract_schema(["Closet", "Nightstand L", "Nightstand R", "Dresser"]),
        container_type_id=ct_types.get("Room"),
    )
    mb_id = api.item_id_from_event(mb_ev)

    api.create_item(mb_id, "Winter Parka", coordinate=abstract_coord("Closet"),
                    category="Clothing", condition="good", tags=["seasonal"])
    api.create_item(mb_id, "Dress Shirts", coordinate=abstract_coord("Closet"),
                    category="Clothing", is_fungible=True, fungible_quantity=5, fungible_unit="pcs",
                    condition="good")
    api.create_item(mb_id, "Shoe Pairs", coordinate=abstract_coord("Closet"),
                    category="Clothing", is_fungible=True, fungible_quantity=4, fungible_unit="pairs",
                    condition="good")
    api.create_item(mb_id, "Phone Charger USB-C", coordinate=abstract_coord("Nightstand L"),
                    category="Electronics", condition="good")
    api.create_item(mb_id, "Reading Glasses", coordinate=abstract_coord("Nightstand L"),
                    category="Personal", condition="good", tags=["fragile"])
    api.create_item(mb_id, "Bedside Novel", coordinate=abstract_coord("Nightstand L"),
                    category="Books", condition="good")
    api.create_item(mb_id, "Lip Balm", coordinate=abstract_coord("Nightstand R"),
                    category="Personal", condition="new", tags=["consumable"])
    api.create_item(mb_id, "Wireless Earbuds", coordinate=abstract_coord("Nightstand R"),
                    category="Electronics", condition="good", acquisition_cost=59.99,
                    currency="USD", tags=["valuable", "warranty"])
    api.create_item(mb_id, "Sleep Mask", coordinate=abstract_coord("Nightstand R"),
                    category="Personal", condition="good")
    api.create_item(mb_id, "T-Shirts", coordinate=abstract_coord("Dresser"),
                    category="Clothing", is_fungible=True, fungible_quantity=15, fungible_unit="pcs",
                    condition="good")
    api.create_item(mb_id, "Sock Pairs", coordinate=abstract_coord("Dresser"),
                    category="Clothing", is_fungible=True, fungible_quantity=20, fungible_unit="pairs",
                    condition="good")
    api.create_item(mb_id, "Jeans", coordinate=abstract_coord("Dresser"),
                    category="Clothing", is_fungible=True, fungible_quantity=4, fungible_unit="pcs",
                    condition="good")

    # ── Kids Room ────────────────────────────────────────────────────────
    print("  Kids Room...")
    kr_ev = api.create_item(
        house_id, "Kids Room",
        is_container=True,
        description="Colorful bedroom with twin beds, shared by two children.",
        location_schema=abstract_schema(["Toy Chest", "Bookshelf", "Closet"]),
        container_type_id=ct_types.get("Room"),
    )
    kr_id = api.item_id_from_event(kr_ev)
    api.upload_image(kr_id, *next_image("Kids room with bunk beds and toy chest"))

    ev = api.create_item(kr_id, "LEGO City Fire Station Set", coordinate=abstract_coord("Toy Chest"),
                    category="Toys", condition="good", acquisition_cost=59.99, currency="USD",
                    tags=["fragile", "valuable"],
                    external_codes=[{"type": "EAN", "value": "5702017116587"}])
    api.upload_image(api.item_id_from_event(ev), *next_image("LEGO Fire Station box"))
    api.create_item(kr_id, "Action Figures", coordinate=abstract_coord("Toy Chest"),
                    category="Toys", is_fungible=True, fungible_quantity=8, fungible_unit="pcs",
                    condition="fair")
    api.create_item(kr_id, "Art Supply Kit", coordinate=abstract_coord("Toy Chest"),
                    category="Crafts", condition="good",
                    description="Markers, colored pencils, crayons, watercolors, and sketchpad.")
    api.create_item(kr_id, "Play-Doh Containers", coordinate=abstract_coord("Toy Chest"),
                    category="Toys", is_fungible=True, fungible_quantity=6, fungible_unit="tubs",
                    condition="good", tags=["consumable"])
    # Children's books
    kid_books = [
        "Where the Wild Things Are", "Goodnight Moon", "The Very Hungry Caterpillar",
        "Green Eggs and Ham", "Charlotte's Web", "Matilda",
        "Charlie and the Chocolate Factory", "The BFG", "James and the Giant Peach",
        "Diary of a Wimpy Kid", "Captain Underpants", "Dog Man",
        "Harry Potter and the Sorcerer's Stone", "Percy Jackson: The Lightning Thief",
        "The Chronicles of Narnia", "A Wrinkle in Time", "The Giving Tree", "Corduroy",
    ]
    for b in kid_books:
        api.create_item(kr_id, b, coordinate=abstract_coord("Bookshelf"),
                        category="Books", condition="good")
    api.create_item(kr_id, "Kids Jackets", coordinate=abstract_coord("Closet"),
                    category="Clothing", is_fungible=True, fungible_quantity=3, fungible_unit="pcs",
                    condition="good", tags=["seasonal"])
    api.create_item(kr_id, "School Uniforms", coordinate=abstract_coord("Closet"),
                    category="Clothing", is_fungible=True, fungible_quantity=5, fungible_unit="sets",
                    condition="good")

    # ── Bathroom ─────────────────────────────────────────────────────────
    print("  Bathroom...")
    bath_ev = api.create_item(
        house_id, "Bathroom",
        is_container=True,
        description="Full bathroom with tub/shower combo, pedestal sink, and medicine cabinet.",
        location_schema=abstract_schema(["Medicine Cabinet", "Under Vanity", "Shower Shelf"]),
        container_type_id=ct_types.get("Room"),
    )
    bath_id = api.item_id_from_event(bath_ev)

    api.create_item(bath_id, "Ibuprofen 200mg Bottle", coordinate=abstract_coord("Medicine Cabinet"),
                    category="Health", condition="good", tags=["consumable", "dangerous"])
    api.create_item(bath_id, "Bandaids Assorted", coordinate=abstract_coord("Medicine Cabinet"),
                    category="Health", is_fungible=True, fungible_quantity=50, fungible_unit="pcs",
                    tags=["consumable"])
    api.create_item(bath_id, "Digital Thermometer", coordinate=abstract_coord("Medicine Cabinet"),
                    category="Health", condition="good")
    api.create_item(bath_id, "Daily Multivitamins", coordinate=abstract_coord("Medicine Cabinet"),
                    category="Health", condition="new", tags=["consumable"])
    api.create_item(bath_id, "Toilet Paper Rolls", coordinate=abstract_coord("Under Vanity"),
                    category="Personal", is_fungible=True, fungible_quantity=12, fungible_unit="rolls",
                    tags=["consumable"])
    api.create_item(bath_id, "Bath Towels", coordinate=abstract_coord("Under Vanity"),
                    category="Personal", is_fungible=True, fungible_quantity=6, fungible_unit="pcs",
                    condition="good")
    api.create_item(bath_id, "Bathroom Cleaning Spray", coordinate=abstract_coord("Under Vanity"),
                    category="Cleaning", condition="good", tags=["consumable", "dangerous"])
    api.create_item(bath_id, "Shampoo Bottles", coordinate=abstract_coord("Shower Shelf"),
                    category="Personal", is_fungible=True, fungible_quantity=2, fungible_unit="bottles",
                    condition="new", tags=["consumable"])
    api.create_item(bath_id, "Conditioner", coordinate=abstract_coord("Shower Shelf"),
                    category="Personal", condition="new", tags=["consumable"])
    api.create_item(bath_id, "Body Wash", coordinate=abstract_coord("Shower Shelf"),
                    category="Personal", condition="new", tags=["consumable"])
    api.create_item(bath_id, "Safety Razor", coordinate=abstract_coord("Shower Shelf"),
                    category="Personal", condition="good", tags=["dangerous"])

    # ── Home Office ──────────────────────────────────────────────────────
    print("  Home Office...")
    off_ev = api.create_item(
        house_id, "Home Office",
        is_container=True,
        description="Converted spare bedroom used as a home office. Standing desk and filing cabinet.",
        location_schema=abstract_schema(["Desk", "Filing Cabinet", "Shelf"]),
        container_type_id=ct_types.get("Room"),
    )
    off_id = api.item_id_from_event(off_ev)
    api.upload_image(off_id, *next_image("Home office desk setup"))

    ev = api.create_item(off_id, '27" 4K Monitor', coordinate=abstract_coord("Desk"),
                    category="Electronics", condition="good", acquisition_cost=349.99,
                    currency="USD", tags=["valuable", "fragile", "warranty"],
                    dimensions=dims(61, 36, 18), weight_grams=4500,
                    external_codes=[{"type": "UPC", "value": "889349765432"}])
    api.upload_image(api.item_id_from_event(ev), *next_image("Dell 27 inch 4K monitor"))
    api.create_item(off_id, "Mechanical Keyboard", coordinate=abstract_coord("Desk"),
                    category="Electronics", condition="good", acquisition_cost=89.99, currency="USD")
    api.create_item(off_id, "Wireless Mouse", coordinate=abstract_coord("Desk"),
                    category="Electronics", condition="good")
    api.create_item(off_id, "USB-C Hub 7-port", coordinate=abstract_coord("Desk"),
                    category="Electronics", condition="good")
    api.create_item(off_id, "LED Desk Lamp", coordinate=abstract_coord("Desk"),
                    category="Office", condition="good")
    api.create_item(off_id, "Spiral Notebooks", coordinate=abstract_coord("Desk"),
                    category="Office", is_fungible=True, fungible_quantity=3, fungible_unit="pcs")

    # Filing Cabinet (sub-container with grid schema)
    fc_ev = api.create_item(off_id, "Filing Cabinet", coordinate=abstract_coord("Filing Cabinet"),
                    is_container=True,
                    location_schema=grid_schema(3, 1, row_labels=["Top", "Middle", "Bottom"]),
                    container_type_id=ct_types.get("Cabinet"),
                    dimensions=dims(38, 102, 46), weight_grams=18000, tags=["heavy"])
    fc_id = api.item_id_from_event(fc_ev)

    api.create_item(fc_id, "Tax Records 2020-2025", coordinate=grid_coord(0, 0),
                    category="Documents", condition="good", tags=["valuable"])
    api.create_item(fc_id, "Insurance Documents", coordinate=grid_coord(0, 0),
                    category="Documents", condition="good", tags=["valuable"])
    api.create_item(fc_id, "Appliance Manuals", coordinate=grid_coord(1, 0),
                    category="Documents", condition="good")
    api.create_item(fc_id, "Printer Paper", coordinate=grid_coord(2, 0),
                    category="Office", is_fungible=True, fungible_quantity=500, fungible_unit="sheets",
                    tags=["consumable"])

    # Shelf items
    for title in ["Python Cookbook", "Clean Code", "Design Patterns",
                   "The Pragmatic Programmer", "SICP", "Algorithms in C"]:
        api.create_item(off_id, title, coordinate=abstract_coord("Shelf"),
                        category="Books", condition="good")
    api.create_item(off_id, "External Hard Drive 2TB", coordinate=abstract_coord("Shelf"),
                    category="Electronics", condition="good", acquisition_cost=69.99,
                    currency="USD", tags=["valuable"])
    api.create_item(off_id, "Wi-Fi Router", coordinate=abstract_coord("Shelf"),
                    category="Electronics", condition="good", tags=["warranty"])
    api.create_item(off_id, "Label Maker", coordinate=abstract_coord("Shelf"),
                    category="Office", condition="good")

    # ── Laundry Room ─────────────────────────────────────────────────────
    print("  Laundry Room...")
    lau_ev = api.create_item(
        house_id, "Laundry Room",
        is_container=True,
        description="Small utility room with washer/dryer stack and wire shelving.",
        location_schema=abstract_schema(["Shelf", "Floor"]),
        container_type_id=ct_types.get("Room"),
    )
    lau_id = api.item_id_from_event(lau_ev)

    api.create_item(lau_id, "Liquid Detergent", coordinate=abstract_coord("Shelf"),
                    category="Cleaning", is_fungible=True, fungible_quantity=2, fungible_unit="bottles",
                    condition="new", tags=["consumable"])
    api.create_item(lau_id, "Dryer Sheets", coordinate=abstract_coord("Shelf"),
                    category="Cleaning", is_fungible=True, fungible_quantity=100, fungible_unit="sheets",
                    tags=["consumable"])
    api.create_item(lau_id, "Steam Iron", coordinate=abstract_coord("Shelf"),
                    category="Kitchen", condition="fair", weight_grams=1400)
    api.create_item(lau_id, "Stain Remover Spray", coordinate=abstract_coord("Shelf"),
                    category="Cleaning", condition="good", tags=["consumable"])
    api.create_item(lau_id, "Wooden Clothespins", coordinate=abstract_coord("Shelf"),
                    is_fungible=True, fungible_quantity=30, fungible_unit="pcs",
                    category="Cleaning")

    # ── Garage ───────────────────────────────────────────────────────────
    print("  Garage...")
    gar_ev = api.create_item(
        house_id, "Garage",
        is_container=True,
        description="Attached two-car garage with pegboard wall, overhead storage rack, and workbench.",
        location_schema=abstract_schema(["Workbench", "Wall Pegboard", "Floor", "Overhead Rack"]),
        container_type_id=ct_types.get("Room"),
        dimensions=dims(600, 280, 600),
    )
    gar_id = api.item_id_from_event(gar_ev)
    api.upload_image(gar_id, *next_image("Garage interior with workbench and tools"))

    # Workbench
    ev = api.create_item(gar_id, "20V Cordless Drill", coordinate=abstract_coord("Workbench"),
                    category="Tools", condition="good", acquisition_cost=89.99, currency="USD",
                    tags=["power-tool", "warranty"],
                    external_codes=[{"type": "UPC", "value": "764666333222"}])
    api.upload_image(api.item_id_from_event(ev), *next_image("DeWalt cordless drill"))
    api.create_item(gar_id, "32-Piece Screwdriver Set", coordinate=abstract_coord("Workbench"),
                    category="Tools", condition="good", tags=["hand-tool"])
    api.create_item(gar_id, "Metric/SAE Wrench Set", coordinate=abstract_coord("Workbench"),
                    category="Tools", condition="good", tags=["hand-tool"],
                    weight_grams=2800)
    api.create_item(gar_id, "Spring Clamps", coordinate=abstract_coord("Workbench"),
                    category="Tools", is_fungible=True, fungible_quantity=4, fungible_unit="pcs",
                    tags=["hand-tool"])
    api.create_item(gar_id, "Spirit Level 24in", coordinate=abstract_coord("Workbench"),
                    category="Tools", condition="good", tags=["hand-tool"])
    api.create_item(gar_id, "Tape Measure 25ft", coordinate=abstract_coord("Workbench"),
                    category="Tools", condition="good", tags=["hand-tool"])

    # Wall Pegboard
    api.create_item(gar_id, "Claw Hammer", coordinate=abstract_coord("Wall Pegboard"),
                    category="Tools", condition="good", tags=["hand-tool"], weight_grams=450)
    api.create_item(gar_id, "Needle-Nose Pliers Set", coordinate=abstract_coord("Wall Pegboard"),
                    category="Tools", condition="good", tags=["hand-tool"])
    api.create_item(gar_id, "Hand Saw", coordinate=abstract_coord("Wall Pegboard"),
                    category="Tools", condition="fair", tags=["hand-tool", "dangerous"])
    api.create_item(gar_id, "Hex Key Set (Allen Wrenches)", coordinate=abstract_coord("Wall Pegboard"),
                    category="Tools", condition="good", tags=["hand-tool"])

    # Floor
    ev = api.create_item(gar_id, "Gas Lawn Mower", coordinate=abstract_coord("Floor"),
                    category="Garden", condition="fair", acquisition_cost=299.99, currency="USD",
                    tags=["heavy", "dangerous", "outdoor"], weight_grams=27000)
    api.upload_image(api.item_id_from_event(ev), *next_image("Honda push lawn mower"))
    api.create_item(gar_id, "Mountain Bike", coordinate=abstract_coord("Floor"),
                    category="Sports", condition="good", acquisition_cost=450.00, currency="USD",
                    tags=["valuable", "outdoor"], weight_grams=12500)
    api.create_item(gar_id, "Snow Blower", coordinate=abstract_coord("Floor"),
                    category="Tools", condition="good", tags=["heavy", "seasonal", "power-tool"],
                    acquisition_cost=599.99, currency="USD", weight_grams=38000)
    api.create_item(gar_id, "Electric Leaf Blower", coordinate=abstract_coord("Floor"),
                    category="Tools", condition="good", tags=["power-tool", "outdoor"],
                    weight_grams=3600)

    # Overhead Rack — 3 storage bins
    rack_ev = api.create_item(gar_id, "Overhead Storage Rack", coordinate=abstract_coord("Overhead Rack"),
                    is_container=True,
                    location_schema=abstract_schema(["Left Bay", "Center Bay", "Right Bay"]),
                    container_type_id=ct_types.get("Rack"))
    rack_id = api.item_id_from_event(rack_ev)

    # Holiday Decorations Bin
    hol_ev = api.create_item(rack_id, "Holiday Decorations Bin", coordinate=abstract_coord("Left Bay"),
                    is_container=True, container_type_id=ct_types.get("Bin"),
                    tags=["seasonal"])
    hol_id = api.item_id_from_event(hol_ev)
    api.create_item(hol_id, "Glass Ornaments", category="Seasonal",
                    is_fungible=True, fungible_quantity=30, fungible_unit="pcs",
                    tags=["fragile", "seasonal"], condition="good")
    api.create_item(hol_id, "String Lights", category="Seasonal",
                    is_fungible=True, fungible_quantity=200, fungible_unit="ft",
                    tags=["seasonal"])
    api.create_item(hol_id, "Evergreen Wreath", category="Seasonal",
                    condition="fair", tags=["seasonal"])
    api.create_item(hol_id, "Christmas Tree Stand", category="Seasonal",
                    condition="good", tags=["seasonal", "heavy"], weight_grams=3000)

    # Camping Gear Bin
    camp_ev = api.create_item(rack_id, "Camping Gear Bin", coordinate=abstract_coord("Center Bay"),
                    is_container=True, container_type_id=ct_types.get("Bin"),
                    tags=["outdoor"])
    camp_id = api.item_id_from_event(camp_ev)
    ev = api.create_item(camp_id, "4-Person Dome Tent", category="Sports",
                    condition="good", tags=["outdoor", "valuable"],
                    acquisition_cost=179.99, currency="USD", weight_grams=3600)
    api.upload_image(api.item_id_from_event(ev), *next_image("Tent packed in carry bag"))
    api.create_item(camp_id, "Sleeping Bags (Rated 30°F)", category="Sports",
                    is_fungible=True, fungible_quantity=2, fungible_unit="pcs",
                    condition="good", tags=["outdoor"])
    api.create_item(camp_id, "Portable Camp Stove", category="Kitchen",
                    condition="good", tags=["outdoor", "dangerous"])
    api.create_item(camp_id, "LED Camping Lantern", category="Emergency",
                    condition="good", tags=["outdoor"])
    api.create_item(camp_id, "48-Qt Cooler", category="Kitchen",
                    condition="good", tags=["outdoor", "heavy"])

    # Sports Equipment Bin
    sport_ev = api.create_item(rack_id, "Sports Equipment Bin", coordinate=abstract_coord("Right Bay"),
                    is_container=True, container_type_id=ct_types.get("Bin"))
    sport_id = api.item_id_from_event(sport_ev)
    api.create_item(sport_id, "Aluminum Baseball Bat", category="Sports",
                    condition="good", tags=["outdoor"])
    api.create_item(sport_id, "Bike Helmets", category="Sports",
                    is_fungible=True, fungible_quantity=3, fungible_unit="pcs",
                    condition="good", tags=["outdoor"])
    api.create_item(sport_id, "Soccer Ball", category="Sports",
                    condition="fair", tags=["outdoor"])
    api.create_item(sport_id, "Tennis Rackets", category="Sports",
                    is_fungible=True, fungible_quantity=2, fungible_unit="pcs",
                    condition="good", tags=["outdoor"])

    # ── Storage Crawl Space ──────────────────────────────────────────────
    print("  Storage Crawl Space...")
    cs_ev = api.create_item(
        house_id, "Storage Crawl Space",
        is_container=True,
        description="Unfinished crawl space under the staircase. Metal shelving and plastic bins organized in a grid layout.",
        location_schema=grid_schema(4, 3,
            row_labels=["Row A", "Row B", "Row C", "Row D"],
            col_labels=["Col 1", "Col 2", "Col 3"]),
        container_type_id=ct_types.get("Room"),
    )
    cs_id = api.item_id_from_event(cs_ev)
    api.upload_image(cs_id, *next_image("Crawl space with labeled storage bins"))

    # [A,1] Shelving Unit
    shelf_ev = api.create_item(cs_id, "Large Shelving Unit", coordinate=grid_coord(0, 0),
                    is_container=True,
                    location_schema=abstract_schema(["Shelf 1", "Shelf 2", "Shelf 3", "Shelf 4", "Shelf 5"]),
                    container_type_id=ct_types.get("Shelf"),
                    dimensions=dims(120, 180, 45), weight_grams=15000, tags=["heavy"])
    shelf_id = api.item_id_from_event(shelf_ev)

    api.create_item(shelf_id, "Photo Albums", coordinate=abstract_coord("Shelf 1"),
                    category="Personal", is_fungible=True, fungible_quantity=3, fungible_unit="albums",
                    condition="fair", tags=["sentimental"])
    api.create_item(shelf_id, "Loose Photos Box", coordinate=abstract_coord("Shelf 1"),
                    category="Personal", condition="fair", tags=["sentimental"])
    api.create_item(shelf_id, "Tax Folders", coordinate=abstract_coord("Shelf 2"),
                    category="Documents", is_fungible=True, fungible_quantity=5, fungible_unit="folders",
                    condition="good", tags=["valuable"])
    api.create_item(shelf_id, "Bank Statements Box", coordinate=abstract_coord("Shelf 2"),
                    category="Documents", condition="good", tags=["valuable"])
    api.create_item(shelf_id, "Yarn Skeins", coordinate=abstract_coord("Shelf 3"),
                    category="Crafts", is_fungible=True, fungible_quantity=12, fungible_unit="skeins",
                    condition="new")
    api.create_item(shelf_id, "Fabric Bolts", coordinate=abstract_coord("Shelf 3"),
                    category="Crafts", is_fungible=True, fungible_quantity=4, fungible_unit="bolts",
                    condition="good")
    api.create_item(shelf_id, "Sewing Kit", coordinate=abstract_coord("Shelf 3"),
                    category="Crafts", condition="good")
    api.create_item(shelf_id, "USB Cables Assorted", coordinate=abstract_coord("Shelf 4"),
                    category="Electronics", is_fungible=True, fungible_quantity=20, fungible_unit="pcs",
                    condition="good")
    api.create_item(shelf_id, "Power Adapters", coordinate=abstract_coord("Shelf 4"),
                    category="Electronics", is_fungible=True, fungible_quantity=8, fungible_unit="pcs",
                    condition="good")
    api.create_item(shelf_id, "Old Smartphone", coordinate=abstract_coord("Shelf 4"),
                    category="Electronics", condition="poor", tags=["vintage"])
    api.create_item(shelf_id, "Old Tablet", coordinate=abstract_coord("Shelf 4"),
                    category="Electronics", condition="poor", tags=["vintage"])
    ev = api.create_item(shelf_id, "Blender", coordinate=abstract_coord("Shelf 5"),
                    category="Kitchen", condition="good", weight_grams=2000)
    api.upload_image(api.item_id_from_event(ev), *next_image("Oster blender in box"))
    api.create_item(shelf_id, "Slow Cooker", coordinate=abstract_coord("Shelf 5"),
                    category="Kitchen", condition="good", weight_grams=4500,
                    acquisition_cost=39.99, currency="USD")
    api.create_item(shelf_id, "Waffle Maker", coordinate=abstract_coord("Shelf 5"),
                    category="Kitchen", condition="good")

    # [A,2] Winter Clothing Tote
    wc_ev = api.create_item(cs_id, "Winter Clothing Tote", coordinate=grid_coord(0, 1),
                    is_container=True, container_type_id=ct_types.get("Tote"),
                    tags=["seasonal"])
    wc_id = api.item_id_from_event(wc_ev)
    api.create_item(wc_id, "Heavy Winter Coats", category="Clothing",
                    is_fungible=True, fungible_quantity=3, fungible_unit="pcs",
                    condition="good", tags=["seasonal"])
    api.create_item(wc_id, "Snow Boots", category="Clothing",
                    is_fungible=True, fungible_quantity=2, fungible_unit="pairs",
                    condition="good", tags=["seasonal"])
    api.create_item(wc_id, "Winter Gloves", category="Clothing",
                    is_fungible=True, fungible_quantity=4, fungible_unit="pairs",
                    condition="good", tags=["seasonal"])

    # [A,3] Baby Items Box
    bb_ev = api.create_item(cs_id, "Baby Items Box", coordinate=grid_coord(0, 2),
                    is_container=True, container_type_id=ct_types.get("Box"),
                    tags=["sentimental"])
    bb_id = api.item_id_from_event(bb_ev)
    api.create_item(bb_id, "Crib Sheets", category="Personal",
                    is_fungible=True, fungible_quantity=4, fungible_unit="sheets",
                    condition="fair", tags=["sentimental"])
    api.create_item(bb_id, "Baby Bottles", category="Kitchen",
                    is_fungible=True, fungible_quantity=6, fungible_unit="bottles",
                    condition="fair")
    api.create_item(bb_id, "Baby Toys", category="Toys",
                    is_fungible=True, fungible_quantity=10, fungible_unit="pcs",
                    condition="fair", tags=["sentimental"])

    # [B,1] Power Tool Case
    pt_ev = api.create_item(cs_id, "Power Tool Case", coordinate=grid_coord(1, 0),
                    is_container=True, container_type_id=ct_types.get("Case"),
                    tags=["heavy"])
    pt_id = api.item_id_from_event(pt_ev)
    ev = api.create_item(pt_id, "Circular Saw", category="Tools",
                    condition="good", tags=["power-tool", "dangerous"],
                    acquisition_cost=129.99, currency="USD", weight_grams=4800)
    api.upload_image(api.item_id_from_event(ev), *next_image("Makita circular saw"))
    api.create_item(pt_id, "Jigsaw", category="Tools",
                    condition="good", tags=["power-tool", "dangerous"],
                    acquisition_cost=69.99, currency="USD")
    api.create_item(pt_id, "Wood Router", category="Tools",
                    condition="good", tags=["power-tool", "dangerous"],
                    acquisition_cost=149.99, currency="USD", weight_grams=3200)
    api.create_item(pt_id, "Drill Bits Assorted", category="Tools",
                    is_fungible=True, fungible_quantity=20, fungible_unit="pcs",
                    condition="good", tags=["hand-tool"])

    # [B,2] Paint Supplies Bucket
    pa_ev = api.create_item(cs_id, "Paint Supplies Bucket", coordinate=grid_coord(1, 1),
                    is_container=True, container_type_id=ct_types.get("Bucket"))
    pa_id = api.item_id_from_event(pa_ev)
    api.create_item(pa_id, "Paint Rollers", category="Tools",
                    is_fungible=True, fungible_quantity=4, fungible_unit="pcs", condition="fair")
    api.create_item(pa_id, "Paint Brushes", category="Tools",
                    is_fungible=True, fungible_quantity=8, fungible_unit="pcs", condition="fair")
    api.create_item(pa_id, "Painter's Tape Rolls", category="Tools",
                    is_fungible=True, fungible_quantity=6, fungible_unit="rolls",
                    condition="good", tags=["consumable"])
    api.create_item(pa_id, "Canvas Drop Cloths", category="Tools",
                    is_fungible=True, fungible_quantity=2, fungible_unit="pcs")

    # [B,3] Plumbing Parts Bin
    pl_ev = api.create_item(cs_id, "Plumbing Parts Bin", coordinate=grid_coord(1, 2),
                    is_container=True, container_type_id=ct_types.get("Bin"))
    pl_id = api.item_id_from_event(pl_ev)
    api.create_item(pl_id, "Assorted Pipe Fittings", category="Tools",
                    is_fungible=True, fungible_quantity=30, fungible_unit="pcs", condition="new")
    api.create_item(pl_id, "Pipe Cutter", category="Tools",
                    condition="good", tags=["hand-tool"])
    api.create_item(pl_id, "Teflon Tape Rolls", category="Tools",
                    is_fungible=True, fungible_quantity=5, fungible_unit="rolls",
                    tags=["consumable"])

    # [C,1] Garden Supplies Crate
    gd_ev = api.create_item(cs_id, "Garden Supplies Crate", coordinate=grid_coord(2, 0),
                    is_container=True, container_type_id=ct_types.get("Crate"),
                    tags=["outdoor"])
    gd_id = api.item_id_from_event(gd_ev)
    api.create_item(gd_id, "Clay Pots", category="Garden",
                    is_fungible=True, fungible_quantity=8, fungible_unit="pcs",
                    condition="good", tags=["fragile", "outdoor"])
    api.create_item(gd_id, "Seed Packets Assorted", category="Garden",
                    is_fungible=True, fungible_quantity=15, fungible_unit="packets",
                    condition="new", tags=["seasonal", "consumable"])
    api.create_item(gd_id, "Garden Trowel", category="Garden",
                    condition="good", tags=["outdoor", "hand-tool"])
    api.create_item(gd_id, "Hose Nozzle", category="Garden",
                    condition="fair", tags=["outdoor"])

    # [C,2] Keepsakes Chest
    kp_ev = api.create_item(cs_id, "Keepsakes Chest", coordinate=grid_coord(2, 1),
                    is_container=True, container_type_id=ct_types.get("Chest"),
                    tags=["sentimental"])
    kp_id = api.item_id_from_event(kp_ev)
    api.upload_image(kp_id, *next_image("Wooden keepsakes chest"))
    api.create_item(kp_id, "High School Yearbooks", category="Personal",
                    is_fungible=True, fungible_quantity=4, fungible_unit="books",
                    condition="fair", tags=["sentimental", "vintage"])
    api.create_item(kp_id, "Sports Trophies", category="Personal",
                    is_fungible=True, fungible_quantity=3, fungible_unit="pcs",
                    condition="fair", tags=["sentimental"])
    api.create_item(kp_id, "Letters and Cards Bundle", category="Personal",
                    condition="fair", tags=["sentimental", "vintage"])

    # [C,3] Luggage Set
    lg_ev = api.create_item(cs_id, "Luggage Set", coordinate=grid_coord(2, 2),
                    is_container=True, container_type_id=ct_types.get("Case"))
    lg_id = api.item_id_from_event(lg_ev)
    api.create_item(lg_id, "Large Suitcase 28in", category="Personal",
                    condition="good", dimensions=dims(50, 71, 30), weight_grams=4500)
    api.create_item(lg_id, "Medium Suitcase 24in", category="Personal",
                    condition="good", dimensions=dims(42, 61, 26), weight_grams=3800)
    api.create_item(lg_id, "Duffle Bag", category="Personal",
                    condition="good", weight_grams=900)
    api.create_item(lg_id, "Carry-On Roller 20in", category="Personal",
                    condition="good", dimensions=dims(35, 55, 23), weight_grams=3200)

    # [D,1] Emergency Kit
    em_ev = api.create_item(cs_id, "Emergency Preparedness Kit", coordinate=grid_coord(3, 0),
                    is_container=True, container_type_id=ct_types.get("Kit"),
                    tags=["dangerous"])
    em_id = api.item_id_from_event(em_ev)
    ev = api.create_item(em_id, "Water Jugs 1-Gallon", category="Emergency",
                    is_fungible=True, fungible_quantity=6, fungible_unit="gallons",
                    condition="new", tags=["consumable", "heavy"])
    api.upload_image(api.item_id_from_event(ev), *next_image("Emergency water jugs"))
    api.create_item(em_id, "MRE Ration Packs", category="Emergency",
                    is_fungible=True, fungible_quantity=24, fungible_unit="packs",
                    condition="new", tags=["consumable"])
    api.create_item(em_id, "Heavy-Duty Flashlights", category="Emergency",
                    is_fungible=True, fungible_quantity=3, fungible_unit="pcs",
                    condition="new")
    api.create_item(em_id, "Hand-Crank Emergency Radio", category="Emergency",
                    condition="new", tags=["valuable"])

    # [D,2] Old Toys Box
    ot_ev = api.create_item(cs_id, "Old Toys Box", coordinate=grid_coord(3, 1),
                    is_container=True, container_type_id=ct_types.get("Box"),
                    tags=["sentimental", "vintage"])
    ot_id = api.item_id_from_event(ot_ev)
    api.create_item(ot_id, "Vintage LEGO Loose Bricks", category="Toys",
                    is_fungible=True, fungible_quantity=200, fungible_unit="pcs",
                    condition="fair", tags=["vintage", "sentimental"])
    api.create_item(ot_id, "Stuffed Animals", category="Toys",
                    is_fungible=True, fungible_quantity=12, fungible_unit="pcs",
                    condition="fair", tags=["sentimental"])
    api.create_item(ot_id, "Jigsaw Puzzles", category="Toys",
                    is_fungible=True, fungible_quantity=5, fungible_unit="boxes",
                    condition="fair")

    # [D,3] Misc Overflow Box
    mo_ev = api.create_item(cs_id, "Misc Overflow Box", coordinate=grid_coord(3, 2),
                    is_container=True, container_type_id=ct_types.get("Box"))
    mo_id = api.item_id_from_event(mo_ev)
    api.create_item(mo_id, "Extension Cords", category="Electronics",
                    is_fungible=True, fungible_quantity=4, fungible_unit="pcs",
                    condition="good")
    api.create_item(mo_id, "Nylon Rope 50ft", category="Tools",
                    condition="good", tags=["outdoor"])
    api.create_item(mo_id, "Zip Ties Assorted", category="Tools",
                    is_fungible=True, fungible_quantity=200, fungible_unit="pcs",
                    condition="new", tags=["consumable"])
    api.create_item(mo_id, "Bungee Cords", category="Tools",
                    is_fungible=True, fungible_quantity=10, fungible_unit="pcs",
                    condition="good")

    # ── Summary ──────────────────────────────────────────────────────────
    total = _counts["containers"] + _counts["items"] + _counts["fungible"]
    print()
    print(f"  Seed complete!")
    print(f"    Containers: {_counts['containers']}")
    print(f"    Items:      {_counts['items']}")
    print(f"    Fungible:   {_counts['fungible']}")
    print(f"    Images:     {_counts['images']}")
    print(f"    Total:      {total}")


def main():
    parser = argparse.ArgumentParser(description="Seed Homorg with test data")
    parser.add_argument("--base-url", default="http://localhost:8080",
                        help="Backend base URL (default: http://localhost:8080)")
    parser.add_argument("--username", default=ADMIN_USER,
                        help=f"Admin username (default: {ADMIN_USER})")
    parser.add_argument("--password", default=None,
                        help="Admin password (prompted if not provided)")
    args = parser.parse_args()

    password = args.password
    if password is None:
        import getpass
        password = getpass.getpass(f"Password for '{args.username}': ")

    print(f"Seeding Homorg at {args.base_url}...")
    start = time.time()
    api = Api(args.base_url)
    api.username = args.username
    api.password = password
    seed(api)
    elapsed = time.time() - start
    print(f"    Time:       {elapsed:.1f}s")


if __name__ == "__main__":
    main()
