#!/usr/bin/env bash
# SpaceSpore - lanceur Linux / macOS (équivalent de run.bat)
#  - Dans un dossier d'installation : lance spacespore-launcher (mises à jour) ou le jeu.
#  - Dans le dépôt source : compile si besoin puis lance le jeu.
cd "$(dirname "$0")" || exit 1
export RUST_BACKTRACE=1

echo "========================================"
echo "   SpaceSpore - Voxel Universe"
echo "========================================"

if [ -x "./spacespore-launcher" ]; then
    exec ./spacespore-launcher
fi
if [ -x "./spacespore" ]; then
    exec ./spacespore
fi

if [ ! -x "target/release/spacespore" ]; then
    echo "Compilation initiale en cours..."
    if ! cargo build --release -p spacespore; then
        echo
        echo "[ERREUR] La compilation a echoue."
        exit 1
    fi
fi

echo
echo "Lancement de SpaceSpore..."
echo
echo "Controles:"
echo "  WASD / Fleches  - Rotation camera"
echo "  Clic droit      - Rotation libre (trackpad Mac : clic a deux doigts)"
echo "  Molette         - Zoom (trackpad : glisser a deux doigts)"
echo "  F2              - Multijoueur"
echo
./target/release/spacespore
code=$?
if [ "$code" -ne 0 ]; then
    echo
    echo "[ERREUR] SpaceSpore s'est arrete avec le code $code."
fi
exit $code
