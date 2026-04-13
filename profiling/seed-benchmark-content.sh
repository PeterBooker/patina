#!/bin/bash
# Seed a realistic WordPress install for the HTTP bench.
#
# Phase 2 of docs/BENCHMARK_PLAN.md. Runs *inside* the profiling php-fpm
# container — expects WP-CLI on PATH, a writable /var/www/html, and a
# reachable mariadb. Called from setup-wordpress.sh after wp core install.
#
# Idempotency: the script wipes existing content first and re-seeds. Running
# it twice is safe, just wastes a few seconds. That's simpler than tracking
# what was imported last time and diffing — and benchmarks want a known
# starting state anyway.
set -euo pipefail

WP="php -d memory_limit=512M /usr/local/bin/wp"
PATH_ARG="--path=/var/www/html"
CONTENT_DIR="/app/profiling/benchmark-content"

# Load WXR pin
# shellcheck disable=SC1091
source "${CONTENT_DIR}/WXR_PIN.env"

echo "=== Installing TwentyTwenty-Five (FSE default) ==="
$WP theme install twentytwentyfive --activate $PATH_ARG

echo "=== Wiping existing content ==="
# Delete every post/page/attachment — the import and generators below will
# re-seed from zero. --force bypasses the trash so ids reset cleanly.
POST_IDS=$($WP post list --format=ids --post_type=any --post_status=any $PATH_ARG || true)
if [ -n "$POST_IDS" ]; then
    # shellcheck disable=SC2086
    $WP post delete $POST_IDS --force $PATH_ARG
fi
COMMENT_IDS=$($WP comment list --format=ids $PATH_ARG || true)
if [ -n "$COMMENT_IDS" ]; then
    # shellcheck disable=SC2086
    $WP comment delete $COMMENT_IDS --force $PATH_ARG
fi
# Leave the admin user in place; delete other generated users.
USER_IDS=$($WP user list --role=author --format=ids $PATH_ARG || true)
if [ -n "$USER_IDS" ]; then
    # shellcheck disable=SC2086
    $WP user delete $USER_IDS --yes --reassign=1 $PATH_ARG
fi

echo "=== Importing canonical WXR fixture (${WXR_REPO}@${WXR_REF}) ==="
WXR_CACHE="/tmp/patina-wxr-${WXR_REF}.xml"
if [ ! -s "$WXR_CACHE" ]; then
    curl -fsSL \
        "https://raw.githubusercontent.com/${WXR_REPO}/${WXR_REF}/${WXR_FILE}" \
        -o "$WXR_CACHE"
fi
# wordpress-importer is required for wp import
$WP plugin install wordpress-importer --activate $PATH_ARG
# Import, mapping unknown authors onto admin so references resolve.
$WP import "$WXR_CACHE" --authors=skip $PATH_ARG || {
    echo "WXR import failed — continuing with generated content only" >&2
}

echo "=== Generating authors ==="
$WP user generate --count=5 --role=author $PATH_ARG

echo "=== Creating block-tier posts ==="
for tier in short medium long; do
    CONTENT=$(cat "${CONTENT_DIR}/posts-${tier}.html")
    COUNT=10
    [ "$tier" = "long" ] && COUNT=5
    for i in $(seq 1 "$COUNT"); do
        $WP post create \
            --post_type=post \
            --post_status=publish \
            --post_title="A ${tier} block post ${i}" \
            --post_name="a-${tier}-block-post-${i}" \
            --post_content="$CONTENT" \
            $PATH_ARG > /dev/null
    done
done

echo "=== Creating classic (non-block) posts ==="
CLASSIC=$(cat "${CONTENT_DIR}/classic-post.html")
for i in 1 2 3; do
    $WP post create \
        --post_type=post \
        --post_status=publish \
        --post_title="A classic HTML post ${i}" \
        --post_name="a-classic-html-post-${i}" \
        --post_content="$CLASSIC" \
        $PATH_ARG > /dev/null
done

echo "=== Creating a stable single-post alias (benchmark entry point) ==="
# The HTTP bench targets fixed slugs — these are the ones k6-workloads.js
# expects in Phase 2+. We create them explicitly so they can't drift.
$WP post create \
    --post_type=post \
    --post_status=publish \
    --post_title="A short block post" \
    --post_name="a-short-block-post" \
    --post_content="$(cat "${CONTENT_DIR}/posts-short.html")" \
    $PATH_ARG > /dev/null
$WP post create \
    --post_type=post \
    --post_status=publish \
    --post_title="A long block post" \
    --post_name="a-long-block-post" \
    --post_content="$(cat "${CONTENT_DIR}/posts-long.html")" \
    $PATH_ARG > /dev/null
$WP post create \
    --post_type=post \
    --post_status=publish \
    --post_title="A classic HTML post" \
    --post_name="a-classic-html-post" \
    --post_content="$CLASSIC" \
    $PATH_ARG > /dev/null

echo "=== Creating pages ==="
$WP post create --post_type=page --post_title="About" --post_name="about" \
    --post_status=publish --post_content="<!-- wp:paragraph --><p>About page.</p><!-- /wp:paragraph -->" \
    $PATH_ARG > /dev/null
$WP post create --post_type=page --post_title="Contact" --post_name="contact" \
    --post_status=publish --post_content="<!-- wp:paragraph --><p>Contact page.</p><!-- /wp:paragraph -->" \
    $PATH_ARG > /dev/null

echo "=== Creating taxonomy terms ==="
for term in announcements perf dev; do
    $WP term create category "$term" --slug="$term" $PATH_ARG 2>/dev/null || true
done
for tag in lorem rust php wordpress bench; do
    $WP term create post_tag "$tag" --slug="$tag" $PATH_ARG 2>/dev/null || true
done

# Assign some posts to the announcements category so /category/announcements/
# has multiple entries in the archive loop.
ANNOUNCE_ID=$($WP term list category --slug=announcements --field=term_id $PATH_ARG)
POST_IDS_FOR_CAT=$($WP post list --format=ids --posts_per_page=5 $PATH_ARG)
for pid in $POST_IDS_FOR_CAT; do
    $WP post term set "$pid" category "$ANNOUNCE_ID" --by=id $PATH_ARG > /dev/null || true
done

# Tag a handful of posts with "perf" so /tag/perf/ exists.
PERF_POSTS=$($WP post list --format=ids --posts_per_page=4 $PATH_ARG)
for pid in $PERF_POSTS; do
    $WP post term set "$pid" post_tag perf --by=slug $PATH_ARG > /dev/null || true
done

echo "=== Generating comments ==="
COMMENTED_POSTS=$($WP post list --format=ids --posts_per_page=8 $PATH_ARG)
for pid in $COMMENTED_POSTS; do
    $WP comment generate --count=5 --post_id="$pid" $PATH_ARG > /dev/null
done
# One post with heavy comment load for the /a-commented-post/ scenario.
HEAVY_ID=$($WP post create \
    --post_type=post --post_status=publish \
    --post_title="A commented post" --post_name="a-commented-post" \
    --post_content="$(cat "${CONTENT_DIR}/posts-medium.html")" \
    --porcelain $PATH_ARG)
$WP comment generate --count=20 --post_id="$HEAVY_ID" $PATH_ARG > /dev/null

echo "=== Nav menu ==="
$WP menu create "Primary" $PATH_ARG 2>/dev/null || true
$WP menu item add-custom primary "Home" "/" $PATH_ARG > /dev/null 2>&1 || true
$WP menu item add-post primary "$HEAVY_ID" $PATH_ARG > /dev/null 2>&1 || true
ABOUT_ID=$($WP post list --post_type=page --name=about --format=ids $PATH_ARG | head -n1)
CONTACT_ID=$($WP post list --post_type=page --name=contact --format=ids $PATH_ARG | head -n1)
[ -n "$ABOUT_ID" ] && $WP menu item add-post primary "$ABOUT_ID" $PATH_ARG > /dev/null 2>&1 || true
[ -n "$CONTACT_ID" ] && $WP menu item add-post primary "$CONTACT_ID" $PATH_ARG > /dev/null 2>&1 || true

echo "=== Flushing rewrites ==="
$WP rewrite flush $PATH_ARG

echo "=== Warming OPcache ==="
for path in / /a-short-block-post/ /a-long-block-post/ /a-classic-html-post/ /a-commented-post/ /category/announcements/ /?s=lorem; do
    curl -s "http://localhost${path}" > /dev/null || true
done

echo ""
echo "=== Benchmark content ready ==="
$WP post list --format=count $PATH_ARG | xargs echo "  posts:"
$WP comment list --format=count $PATH_ARG | xargs echo "  comments:"
$WP user list --format=count $PATH_ARG | xargs echo "  users:"
