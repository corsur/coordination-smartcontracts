#!/usr/bin/env bash
set -euo pipefail

# check-upgrade-authority.sh
#
# Verifies the upgrade authority of a deployed Solana program.
# - Devnet: logs the authority (informational).
# - Mainnet-beta: compares against EXPECTED_MULTISIG_PUBKEY env var. Fails on mismatch.

usage() {
  cat <<EOF
Usage: $0 --cluster <devnet|mainnet-beta> --program-id <PUBKEY>

Options:
  --cluster       Solana cluster: devnet or mainnet-beta
  --program-id    The program's public key

Environment variables:
  EXPECTED_MULTISIG_PUBKEY  (optional on devnet, checked on mainnet-beta)
                            The expected Squads multisig upgrade authority.
                            On mainnet-beta: mismatch exits 1.
                            If unset on mainnet-beta: warns and exits 0.
EOF
  exit 1
}

CLUSTER=""
PROGRAM_ID=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cluster)
      CLUSTER="$2"
      shift 2
      ;;
    --program-id)
      PROGRAM_ID="$2"
      shift 2
      ;;
    --help|-h)
      usage
      ;;
    *)
      echo "ERROR: Unknown argument: $1"
      usage
      ;;
  esac
done

if [[ -z "$CLUSTER" || -z "$PROGRAM_ID" ]]; then
  echo "ERROR: --cluster and --program-id are required."
  usage
fi

if [[ "$CLUSTER" != "devnet" && "$CLUSTER" != "mainnet-beta" ]]; then
  echo "ERROR: --cluster must be 'devnet' or 'mainnet-beta', got '$CLUSTER'."
  exit 1
fi

echo "Checking upgrade authority for program $PROGRAM_ID on $CLUSTER..."

# Fetch program info. Capture both stdout and stderr so we can detect errors.
OUTPUT=""
if ! OUTPUT=$(solana program show "$PROGRAM_ID" --url "$CLUSTER" 2>&1); then
  echo "ERROR: Failed to query program $PROGRAM_ID on $CLUSTER."
  echo "solana program show output:"
  echo "$OUTPUT"
  exit 1
fi

# Parse the Authority field from the output.
AUTHORITY=$(echo "$OUTPUT" | grep -E "^Authority:" | awk '{print $2}')

if [[ -z "$AUTHORITY" ]]; then
  echo "ERROR: Could not parse Authority field from program info."
  echo "Full output:"
  echo "$OUTPUT"
  exit 1
fi

echo "Program $PROGRAM_ID upgrade authority: $AUTHORITY"

if [[ "$CLUSTER" == "devnet" ]]; then
  echo "INFO: Devnet deployment — authority check is informational only."
  exit 0
fi

# Mainnet-beta: compare against expected multisig.
if [[ -z "${EXPECTED_MULTISIG_PUBKEY:-}" ]]; then
  echo "WARNING: EXPECTED_MULTISIG_PUBKEY is not set. Skipping mainnet authority verification."
  echo "WARNING: Set this variable to enforce the Squads multisig check."
  exit 0
fi

if [[ "$AUTHORITY" != "$EXPECTED_MULTISIG_PUBKEY" ]]; then
  echo "ERROR: Upgrade authority mismatch on mainnet-beta!"
  echo "  Expected: $EXPECTED_MULTISIG_PUBKEY"
  echo "  Actual:   $AUTHORITY"
  echo "This program's upgrade authority is NOT the expected Squads multisig."
  echo "Deployment safety check FAILED."
  exit 1
fi

echo "OK: Upgrade authority matches expected Squads multisig ($EXPECTED_MULTISIG_PUBKEY)."
exit 0
