#!/usr/bin/env bash
# setup-build-volume.sh
# Creates a dedicated LVM volume for homorg build tooling (Flutter, Android SDK,
# Gradle cache, Dart pub cache) and migrates existing data into it.
#
# Usage: sudo bash setup-build-volume.sh [SIZE_GB]
#   SIZE_GB  — size of new LV in gigabytes (default: 15)
#
# If the VG has no free space, the script will shrink 'lcsas-test' (which has
# plenty of headroom) to carve out the needed PEs.
#
# What it does:
#   1. Creates /dev/<vg>/homorg-build (ext4, mounted at /mnt/homorg-build)
#   2. Adds entry to /etc/fstab
#   3. Moves any existing data from /var/tmp/{flutter,android-sdk,pub-cache}
#      and /mnt/lcsas-test/gradle into the new volume
#   4. Creates symlinks so existing paths keep working
#   5. Prints a shell snippet to add to ~/.zshrc

set -euo pipefail

# ── Config ───────────────────────────────────────────────────────────────────
SIZE_GB="${1:-15}"
MOUNT="/mnt/homorg-build"
LV_NAME="homorg-build"

# Donor LV to shrink if VG has no free space (must have enough headroom)
DONOR_LV="lcsas-test"
DONOR_MOUNT="/mnt/lcsas-test"

# Directories to migrate: old_path → subdir inside new volume
declare -A MIGRATE=(
    ["/var/tmp/flutter"]="flutter"
    ["/var/tmp/android-sdk"]="android-sdk"
    ["/var/tmp/pub-cache"]="pub-cache"
    ["/mnt/lcsas-test/gradle"]="gradle"
)

# ── Guards ───────────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    echo "ERROR: run as root (sudo bash $0)" >&2
    exit 1
fi

command -v rsync >/dev/null || { apt-get install -y rsync; }

# ── Discover VG ──────────────────────────────────────────────────────────────
VG=$(vgs --noheadings -o vg_name 2>/dev/null | awk 'NR==1{print $1}')
if [[ -z "$VG" ]]; then
    echo "ERROR: no LVM volume group found" >&2
    exit 1
fi
echo "Volume group: $VG"

LV_PATH="/dev/$VG/$LV_NAME"
DONOR_PATH="/dev/$VG/$DONOR_LV"

# ── Create LV (or skip if it already exists) ─────────────────────────────────
if lvs "$LV_PATH" &>/dev/null; then
    echo "LV $LV_PATH already exists — skipping lvcreate"
else
    # Check VG free space
    FREE_GB=$(vgs --noheadings --units g -o vg_free "$VG" 2>/dev/null | tr -d ' gG')
    FREE_INT=${FREE_GB%.*}
    echo "Free PEs in $VG: ${FREE_GB} GiB"

    if (( FREE_INT < SIZE_GB )); then
        echo "Not enough free space in VG — shrinking $DONOR_LV to make room…"

        # Verify donor LV exists and has enough free space inside it
        if ! lvs "$DONOR_PATH" &>/dev/null; then
            echo "ERROR: donor LV $DONOR_PATH not found" >&2
            exit 1
        fi

        DONOR_SIZE_GB=$(lvs --noheadings --units g -o lv_size "$DONOR_PATH" 2>/dev/null | tr -d ' gG')
        DONOR_SIZE_INT=${DONOR_SIZE_GB%.*}
        # Leave at least SIZE_GB + 5G headroom in the donor
        MIN_DONOR=$(( SIZE_GB + 5 ))
        NEW_DONOR_SIZE=$(( DONOR_SIZE_INT - SIZE_GB ))

        if (( NEW_DONOR_SIZE < MIN_DONOR )); then
            echo "ERROR: $DONOR_LV is only ${DONOR_SIZE_GB}G; shrinking by ${SIZE_GB}G would leave only ${NEW_DONOR_SIZE}G" >&2
            exit 1
        fi

        echo "  $DONOR_LV: ${DONOR_SIZE_GB}G → ${NEW_DONOR_SIZE}G (shrinking by ${SIZE_GB}G)"

        # Must unmount to shrink filesystem safely
        if mountpoint -q "$DONOR_MOUNT"; then
            echo "  Unmounting $DONOR_MOUNT…"
            umount "$DONOR_MOUNT"
        fi

        echo "  Checking filesystem on $DONOR_PATH…"
        e2fsck -f -y "$DONOR_PATH"

        echo "  Resizing filesystem to ${NEW_DONOR_SIZE}G…"
        resize2fs "$DONOR_PATH" "${NEW_DONOR_SIZE}G"

        echo "  Shrinking LV to ${NEW_DONOR_SIZE}G…"
        lvreduce -L "${NEW_DONOR_SIZE}G" "$DONOR_PATH"

        echo "  Re-mounting $DONOR_MOUNT…"
        mount "$DONOR_MOUNT"

        echo "  Done. Freed ${SIZE_GB}G from $DONOR_LV."
    fi

    echo "Creating LV ${LV_NAME} (${SIZE_GB}G)…"
    lvcreate -L "${SIZE_GB}G" -n "$LV_NAME" "$VG"
    echo "Formatting as ext4…"
    mkfs.ext4 -L homorg-build "$LV_PATH"
fi

# ── Mount ────────────────────────────────────────────────────────────────────
mkdir -p "$MOUNT"

if ! grep -q "$LV_PATH" /etc/fstab; then
    echo "Adding $MOUNT to /etc/fstab…"
    echo "$LV_PATH  $MOUNT  ext4  defaults,noatime  0 2" >> /etc/fstab
else
    echo "/etc/fstab entry already present"
fi

if ! mountpoint -q "$MOUNT"; then
    echo "Mounting $MOUNT…"
    mount "$MOUNT"
else
    echo "$MOUNT already mounted"
fi

# ── Migrate existing data ─────────────────────────────────────────────────────
for OLD_PATH in "${!MIGRATE[@]}"; do
    SUBDIR="${MIGRATE[$OLD_PATH]}"
    NEW_PATH="$MOUNT/$SUBDIR"

    if [[ -L "$OLD_PATH" ]]; then
        EXISTING_TARGET=$(readlink -f "$OLD_PATH")
        if [[ "$EXISTING_TARGET" == "$NEW_PATH" ]]; then
            echo "  $OLD_PATH already symlinked to $NEW_PATH — skipping"
        else
            echo "  WARNING: $OLD_PATH is a symlink to $EXISTING_TARGET (not $NEW_PATH) — skipping"
        fi
        continue
    fi

    mkdir -p "$NEW_PATH"

    if [[ -d "$OLD_PATH" && -n "$(ls -A "$OLD_PATH" 2>/dev/null)" ]]; then
        echo "  Migrating $OLD_PATH → $NEW_PATH …"
        rsync -a --remove-source-files "$OLD_PATH/" "$NEW_PATH/"
        find "$OLD_PATH" -depth -type d -empty -delete 2>/dev/null || true
    else
        echo "  $OLD_PATH empty or missing — target dir created for future use"
    fi

    # Replace source with symlink regardless (even if dir was empty)
    rm -rf "$OLD_PATH"
    ln -s "$NEW_PATH" "$OLD_PATH"
    echo "    symlink: $OLD_PATH → $NEW_PATH"
done

# ── Fix ownership ─────────────────────────────────────────────────────────────
# Flutter and Android SDK should be readable by the regular user
REAL_USER="${SUDO_USER:-$(logname 2>/dev/null || echo "")}"
if [[ -n "$REAL_USER" ]]; then
    echo "Setting ownership of $MOUNT to $REAL_USER…"
    chown -R "$REAL_USER:$REAL_USER" "$MOUNT"
fi

# ── Print env snippet ─────────────────────────────────────────────────────────
cat <<'SNIPPET'

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Add this to ~/.zshrc so these paths are set in every session:

  # Homorg build tooling
  export FLUTTER_ROOT="/mnt/homorg-build/flutter"
  export ANDROID_HOME="/mnt/homorg-build/android-sdk"
  export ANDROID_SDK_ROOT="/mnt/homorg-build/android-sdk"
  export PUB_CACHE="/mnt/homorg-build/pub-cache"
  export GRADLE_USER_HOME="/mnt/homorg-build/gradle"
  export PATH="$FLUTTER_ROOT/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
SNIPPET

echo ""
echo "All done. $MOUNT:"
df -h "$MOUNT"
