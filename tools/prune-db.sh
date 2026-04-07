#!/usr/bin/env bash
#
# prune-db.sh — Shrink the park database by removing non-elite genotypes
# from completed evolutions.
#
# Keeps:
#   - All genotypes from RUNNING evolutions (untouched)
#   - Tagged genotypes (creature_tags)
#   - Top N by fitness per (evolution, island, generation)
#   - All genotypes from the final generation of each completed evolution
#
# Usage:
#   tools/prune-db.sh [--keep N] [--dry-run] [DB_PATH]
#
#   --keep N     Keep top-N per island per generation (default: 5)
#   --dry-run    Report what would be deleted without changing anything
#   DB_PATH      Path to SQLite database (default: park.db)

set -euo pipefail

KEEP=5
DRY_RUN=0
DB_PATH="park.db"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep)    KEEP="$2"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        *)         DB_PATH="$1"; shift ;;
    esac
done

if [[ ! -f "$DB_PATH" ]]; then
    echo "Error: database not found at $DB_PATH" >&2
    exit 1
fi

sql() { sqlite3 -noheader -batch -cmd ".timeout 30000" "$DB_PATH" "$1"; }

SIZE_BEFORE=$(stat -f%z "$DB_PATH" 2>/dev/null || stat -c%s "$DB_PATH")

echo "Database: $DB_PATH ($(numfmt --to=iec "$SIZE_BEFORE" 2>/dev/null || echo "${SIZE_BEFORE} bytes"))"
echo "Keeping top-$KEEP per island per generation"
echo ""

TOTAL=$(sql "SELECT COUNT(*) FROM genotypes;")
COMPLETED_EVOS=$(sql "SELECT GROUP_CONCAT(id) FROM evolutions WHERE status != 'running' AND current_gen = -1;")

if [[ -z "$COMPLETED_EVOS" ]]; then
    echo "No completed evolutions to prune."
    exit 0
fi

echo "Completed evolutions to prune: $COMPLETED_EVOS"

IN_COMPLETED=$(sql "SELECT COUNT(*) FROM genotypes WHERE evolution_id IN ($COMPLETED_EVOS);")

# The keeper query: tagged + top-N per island/gen + final generation
KEEP_CTE="
    WITH completed AS (
        SELECT id FROM evolutions WHERE status != 'running' AND current_gen = -1
    ),
    max_gens AS (
        SELECT evolution_id, MAX(generation) as max_gen
        FROM genotypes
        WHERE evolution_id IN (SELECT id FROM completed)
        GROUP BY evolution_id
    ),
    final_gen AS (
        SELECT g.id FROM genotypes g
        JOIN max_gens mg ON g.evolution_id = mg.evolution_id AND g.generation = mg.max_gen
    ),
    tagged AS (
        SELECT DISTINCT genotype_id as id FROM creature_tags
    ),
    ranked AS (
        SELECT g.id,
               ROW_NUMBER() OVER (
                   PARTITION BY g.evolution_id, g.island_id, g.generation
                   ORDER BY g.fitness DESC NULLS LAST
               ) as rn
        FROM genotypes g
        WHERE g.evolution_id IN (SELECT id FROM completed)
    ),
    top_n AS (
        SELECT id FROM ranked WHERE rn <= ${KEEP}
    ),
    keep_ids AS (
        SELECT id FROM final_gen
        UNION SELECT id FROM tagged
        UNION SELECT id FROM top_n
    )
"

WOULD_KEEP=$(sql "
    ${KEEP_CTE}
    SELECT COUNT(*) FROM keep_ids
    WHERE id IN (SELECT id FROM genotypes WHERE evolution_id IN (SELECT id FROM completed));
")

WOULD_DELETE=$((IN_COMPLETED - WOULD_KEEP))

echo ""
echo "Genotypes in completed evolutions: $IN_COMPLETED"
echo "Would keep:   $WOULD_KEEP"
echo "Would delete: $WOULD_DELETE"

if [[ "$WOULD_DELETE" -le 0 ]]; then
    echo "Nothing to prune."
    exit 0
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
    echo ""
    echo "(dry run — no changes made)"
    exit 0
fi

echo ""
read -rp "Proceed with deletion? [y/N] " CONFIRM
if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
    echo "Aborted."
    exit 1
fi

# Safety: back up first
BACKUP="${DB_PATH}.backup-$(date +%Y%m%d-%H%M%S)"
echo "Backing up to $BACKUP ..."
cp "$DB_PATH" "$BACKUP"

echo "Deleting $WOULD_DELETE genotypes ..."

# Build the DELETE target as a subquery
DELETE_TARGET="
    ${KEEP_CTE}
    SELECT id FROM genotypes
    WHERE evolution_id IN (SELECT id FROM completed)
      AND id NOT IN (SELECT id FROM keep_ids)
"

sql "DELETE FROM tasks WHERE genotype_id IN (${DELETE_TARGET});"
sql "DELETE FROM genotypes WHERE id IN (${DELETE_TARGET});"

REMAINING=$(sql "SELECT COUNT(*) FROM genotypes;")
echo "Genotypes remaining: $REMAINING (was $TOTAL)"

echo "Running VACUUM (this may take a while) ..."
sql "VACUUM;"

SIZE_AFTER=$(stat -f%z "$DB_PATH" 2>/dev/null || stat -c%s "$DB_PATH")
echo ""
echo "Done!"
echo "  Before: $(numfmt --to=iec "$SIZE_BEFORE" 2>/dev/null || echo "$SIZE_BEFORE bytes")"
echo "  After:  $(numfmt --to=iec "$SIZE_AFTER" 2>/dev/null || echo "$SIZE_AFTER bytes")"
echo "  Saved:  $(numfmt --to=iec $((SIZE_BEFORE - SIZE_AFTER)) 2>/dev/null || echo "$((SIZE_BEFORE - SIZE_AFTER)) bytes")"
echo "  Backup: $BACKUP"
