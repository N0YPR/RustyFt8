#!/bin/bash

# ============================================================================
# Docker Utilities
# ============================================================================
# A collection of Docker-related utility functions for building and managing
# container images with intelligent caching and registry integration.
# ============================================================================

# Check for required dependencies
# Returns: 0 if all dependencies are present, 1 otherwise
check_dependencies() {
    local missing=()

    # Check for docker
    if ! command -v docker &> /dev/null; then
        missing+=("docker")
    fi

    # Check for git (needed for repo detection)
    if ! command -v git &> /dev/null; then
        missing+=("git")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "Error: Missing required dependencies: ${missing[*]}" >&2
        echo "Please install the missing tools and try again." >&2
        return 1
    fi

    return 0
}

# Retry a command with exponential backoff
# Args:
#   $1 - Maximum number of retries
#   $2... - Command to execute
# Returns: Exit code of the command
retry_with_backoff() {
    local max_retries="${1}"
    shift
    local cmd=("$@")
    local attempt=1
    local delay=2

    while [[ $attempt -le $max_retries ]]; do
        if "${cmd[@]}"; then
            return 0
        fi

        if [[ $attempt -lt $max_retries ]]; then
            echo "Attempt $attempt failed. Retrying in ${delay}s..." >&2
            sleep "$delay"
            delay=$((delay * 2))  # Exponential backoff: 2s, 4s, 8s...
        fi

        attempt=$((attempt + 1))
    done

    echo "All $max_retries attempts failed." >&2
    return 1
}

# Calculate SHA256 hash of Dockerfile contents
# Args:
#   $1 - Path to Dockerfile (optional, defaults to ./Dockerfile)
#   $2 - Hash length (optional, defaults to 12 for short hash, use 64 for full hash)
# Returns:
#   Prints the SHA256 hash (or truncated short hash) to stdout
#   Exit code 0 on success, 1 on error
calculate_dockerfile_hash() {
    local dockerfile_path="${1:-./Dockerfile}"
    local hash_length="${2:-12}"

    if [[ ! -f "$dockerfile_path" ]]; then
        echo "Error: Dockerfile not found at $dockerfile_path" >&2
        return 1
    fi

    # Calculate SHA256 hash of the file contents
    local full_hash
    if command -v sha256sum &> /dev/null; then
        full_hash=$(sha256sum "$dockerfile_path" | awk '{print $1}')
    elif command -v shasum &> /dev/null; then
        full_hash=$(shasum -a 256 "$dockerfile_path" | awk '{print $1}')
    else
        echo "Error: Neither sha256sum nor shasum command found" >&2
        return 1
    fi

    # Truncate to requested length
    echo "${full_hash:0:$hash_length}"
}

# Auto-detect repository name from git remote
detect_repo_name() {
    if git rev-parse --git-dir > /dev/null 2>&1; then
        local remote_url=$(git remote get-url origin 2>/dev/null || echo "")
        if [[ -n "$remote_url" ]]; then
            # Extract repo name from URL (handles both HTTPS and SSH formats)
            # Examples:
            #   https://github.com/user/repo.git -> repo
            #   git@github.com:user/repo.git -> repo
            echo "$remote_url" | sed -E 's#.*/([^/]+)\.git$#\1#; s#.*/([^/]+)$#\1#' | tr '[:upper:]' '[:lower:]'
            return 0
        fi
    fi
    echo "rvbuildlog3"  # Fallback default
}

# Auto-detect GitHub username from git remote
detect_github_username() {
    if git rev-parse --git-dir > /dev/null 2>&1; then
        local remote_url=$(git remote get-url origin 2>/dev/null || echo "")
        if [[ -n "$remote_url" ]]; then
            # Extract username from URL (handles both HTTPS and SSH formats)
            # Examples:
            #   https://github.com/user/repo.git -> user
            #   git@github.com:user/repo.git -> user
            echo "$remote_url" | sed -E 's#.*github\.com[:/]([^/]+)/.*#\1#' | tr '[:upper:]' '[:lower:]'
            return 0
        fi
    fi
    echo ""  # No fallback - let REGISTRY default handle it
}

# Get current git branch name, sanitized for Docker tag
get_branch_suffix() {
    if git rev-parse --git-dir > /dev/null 2>&1; then
        local branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
        # Only add branch suffix if not on main/master
        if [[ -n "$branch" ]] && [[ "$branch" != "main" ]] && [[ "$branch" != "master" ]]; then
            # Sanitize branch name for Docker tag (replace / and other invalid chars with -)
            echo "-$(echo "$branch" | sed 's/[^a-zA-Z0-9._-]/-/g')"
            return 0
        fi
    fi
    echo ""  # No suffix for main/master branches
}

# Pull or build Docker image with intelligent caching
# Environment variables:
#   IMAGE_NAME - Image name (default: auto-detected from git)
#   REGISTRY - Registry URL (default: ghcr.io/<username>)
#   TARGET - Build target (default: devcontainer)
#   PLATFORM - Platform (default: auto-detected)
#   IMAGE_TAG - Image tag (default: Dockerfile hash)
#   PULL_RETRIES - Number of pull retries (default: 3)
pull_or_build() {
    set -e

    # Check dependencies first
    check_dependencies || return 1

    # Configuration
    local PULL_RETRIES="${PULL_RETRIES:-3}"
    local BASE_IMAGE_NAME="${IMAGE_NAME:-$(detect_repo_name)}"
    local GITHUB_USER=$(detect_github_username)
    local REGISTRY="${REGISTRY:-${GITHUB_USER:+ghcr.io/$GITHUB_USER}}"
    local TARGET="${TARGET:-devcontainer}"
    local PLATFORM="${PLATFORM:-linux/$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')}"
    local BRANCH_SUFFIX=$(get_branch_suffix)

    # Include target in image name
    local IMAGE_NAME="${BASE_IMAGE_NAME}-${TARGET}"

    # Calculate Dockerfile hash for tagging
    local DOCKERFILE_HASH=$(calculate_dockerfile_hash ./Dockerfile)
    local IMAGE_TAG="${IMAGE_TAG:-latest}"

    # Construct full image name with branch suffix
    local FULL_IMAGE_NAME
    if [[ -n "$REGISTRY" ]]; then
        FULL_IMAGE_NAME="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}${BRANCH_SUFFIX}"
    else
        FULL_IMAGE_NAME="${IMAGE_NAME}:${IMAGE_TAG}${BRANCH_SUFFIX}"
    fi

    echo "========================================="
    echo "Pull or Build Docker Image"
    echo "========================================="
    echo "Image Name: ${FULL_IMAGE_NAME}"
    echo "Target: ${TARGET}"
    echo "Platform: ${PLATFORM}"
    echo "Dockerfile Hash: ${DOCKERFILE_HASH}"
    if [[ -n "$BRANCH_SUFFIX" ]]; then
        echo "Branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    fi
    echo "========================================="

    # Try to pull the image with retry logic
    echo ""
    echo "Attempting to pull image (up to ${PULL_RETRIES} attempts)..."
    if retry_with_backoff "$PULL_RETRIES" docker pull "${FULL_IMAGE_NAME}"; then
        echo "✓ Successfully pulled ${FULL_IMAGE_NAME}"

        # Tag with additional aliases for convenience
        docker tag "${FULL_IMAGE_NAME}" "${IMAGE_NAME}:${IMAGE_TAG}"
        docker tag "${FULL_IMAGE_NAME}" "${IMAGE_NAME}:${DOCKERFILE_HASH}${BRANCH_SUFFIX}"
        echo "✓ Tagged as ${IMAGE_NAME}:${IMAGE_TAG} and ${IMAGE_NAME}:${DOCKERFILE_HASH}${BRANCH_SUFFIX}"

        return 0
    else
        echo "✗ Pull failed or image not found in registry"
        echo ""
        echo "Building image locally..."

        # Build the image
        docker build \
            --target "${TARGET}" \
            --platform "${PLATFORM}" \
            -t "${FULL_IMAGE_NAME}" \
            -t "${IMAGE_NAME}:${IMAGE_TAG}" \
            -t "${IMAGE_NAME}:${DOCKERFILE_HASH}${BRANCH_SUFFIX}" \
            .

        echo ""
        echo "✓ Successfully built ${FULL_IMAGE_NAME}"
        echo "✓ Tagged as ${IMAGE_NAME}:${IMAGE_TAG} and ${IMAGE_NAME}:${DOCKERFILE_HASH}${BRANCH_SUFFIX}"

        return 0
    fi
}

# Push Docker image to registry
# Args:
#   $1 - Build target (optional, defaults to TARGET env var or 'devcontainer')
# Environment variables:
#   IMAGE_NAME - Image name (default: auto-detected from git)
#   REGISTRY - Registry URL (default: ghcr.io/<username>)
#   IMAGE_TAG - Image tag (default: Dockerfile hash)
push_image() {
    set -e

    # Check dependencies first
    check_dependencies || return 1

    # Configuration
    local BASE_IMAGE_NAME="${IMAGE_NAME:-$(detect_repo_name)}"
    local GITHUB_USER=$(detect_github_username)
    local REGISTRY="${REGISTRY:-${GITHUB_USER:+ghcr.io/$GITHUB_USER}}"
    local TARGET="${1:-${TARGET:-devcontainer}}"
    local BRANCH_SUFFIX=$(get_branch_suffix)

    # Include target in image name
    local IMAGE_NAME="${BASE_IMAGE_NAME}-${TARGET}"

    # Calculate Dockerfile hash for tagging
    local DOCKERFILE_HASH=$(calculate_dockerfile_hash ./Dockerfile)
    local IMAGE_TAG="${IMAGE_TAG:-latest}"

    # Validate registry is configured
    if [[ -z "$REGISTRY" ]]; then
        echo "Error: REGISTRY is not configured and could not be auto-detected" >&2
        echo "Please set REGISTRY environment variable (e.g., REGISTRY=ghcr.io/username)" >&2
        return 1
    fi

    # Construct full image name with branch suffix
    local FULL_IMAGE_NAME="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}${BRANCH_SUFFIX}"

    echo "========================================="
    echo "Push Docker Image"
    echo "========================================="
    echo "Image Name: ${FULL_IMAGE_NAME}"
    echo "Target: ${TARGET}"
    echo "Registry: ${REGISTRY}"
    echo "Dockerfile Hash: ${DOCKERFILE_HASH}"
    if [[ -n "$BRANCH_SUFFIX" ]]; then
        echo "Branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    fi
    echo "========================================="
    echo ""

    # Push the image
    echo "Pushing image to registry..."
    docker push "${FULL_IMAGE_NAME}"
    echo ""
    echo "✓ Successfully pushed ${FULL_IMAGE_NAME}"

    return 0
}

# Check if Docker image exists in registry
# Args:
#   $1 - Build target (optional, defaults to TARGET env var or 'devcontainer')
# Environment variables:
#   IMAGE_NAME - Image name (default: auto-detected from git)
#   REGISTRY - Registry URL (default: ghcr.io/<username>)
#   IMAGE_TAG - Image tag (default: Dockerfile hash)
# Returns:
#   Exit code 0 if image exists, 1 if not found
image_exists() {
    # Configuration
    local BASE_IMAGE_NAME="${IMAGE_NAME:-$(detect_repo_name)}"
    local GITHUB_USER=$(detect_github_username)
    local REGISTRY="${REGISTRY:-${GITHUB_USER:+ghcr.io/$GITHUB_USER}}"
    local TARGET="${1:-${TARGET:-devcontainer}}"
    local BRANCH_SUFFIX=$(get_branch_suffix)

    # Include target in image name
    local IMAGE_NAME="${BASE_IMAGE_NAME}-${TARGET}"

    # Calculate Dockerfile hash for tagging
    local DOCKERFILE_HASH=$(calculate_dockerfile_hash ./Dockerfile)
    local IMAGE_TAG="${IMAGE_TAG:-latest}"

    # Validate registry is configured
    if [[ -z "$REGISTRY" ]]; then
        echo "Error: REGISTRY is not configured and could not be auto-detected" >&2
        echo "Please set REGISTRY environment variable (e.g., REGISTRY=ghcr.io/username)" >&2
        return 2
    fi

    # Construct full image name with branch suffix
    local FULL_IMAGE_NAME="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}${BRANCH_SUFFIX}"

    echo "Checking if image exists: ${FULL_IMAGE_NAME}"

    # Try to inspect the manifest (doesn't download layers)
    if docker manifest inspect "${FULL_IMAGE_NAME}" > /dev/null 2>&1; then
        echo "✓ Image exists in registry"
        return 0
    else
        echo "✗ Image not found in registry"
        return 1
    fi
}

# Build and push Docker image for multiple architectures
# Args:
#   $1 - Build target (optional, defaults to TARGET env var or 'devcontainer')
# Environment variables:
#   IMAGE_NAME - Image name (default: auto-detected from git)
#   REGISTRY - Registry URL (default: ghcr.io/<username>)
#   PLATFORMS - Platforms to build for (default: linux/amd64,linux/arm64)
#   IMAGE_TAG - Image tag (default: Dockerfile hash)
build_and_push() {
    set -e

    # Check dependencies first
    check_dependencies || return 1

    # Check for docker buildx
    if ! docker buildx version &> /dev/null; then
        echo "Error: docker buildx is not available" >&2
        echo "Please install Docker Buildx and try again." >&2
        return 1
    fi

    # Configuration
    local BASE_IMAGE_NAME="${IMAGE_NAME:-$(detect_repo_name)}"
    local GITHUB_USER=$(detect_github_username)
    local REGISTRY="${REGISTRY:-${GITHUB_USER:+ghcr.io/$GITHUB_USER}}"
    local TARGET="${1:-${TARGET:-devcontainer}}"
    local PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"
    local BRANCH_SUFFIX=$(get_branch_suffix)

    # Include target in image name
    local IMAGE_NAME="${BASE_IMAGE_NAME}-${TARGET}"

    # Calculate Dockerfile hash for tagging
    local DOCKERFILE_HASH=$(calculate_dockerfile_hash ./Dockerfile)
    local IMAGE_TAG="${IMAGE_TAG:-latest}"

    # Validate registry is configured
    if [[ -z "$REGISTRY" ]]; then
        echo "Error: REGISTRY is not configured and could not be auto-detected" >&2
        echo "Please set REGISTRY environment variable (e.g., REGISTRY=ghcr.io/username)" >&2
        return 1
    fi

    # Construct image tags
    local LATEST_TAG="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}${BRANCH_SUFFIX}"
    local HASH_TAG="${REGISTRY}/${IMAGE_NAME}:${DOCKERFILE_HASH}${BRANCH_SUFFIX}"

    echo "========================================="
    echo "Build and Push Multi-Architecture Image"
    echo "========================================="
    echo "Image Name: ${IMAGE_NAME}"
    echo "Latest Tag: ${LATEST_TAG}"
    echo "Hash Tag: ${HASH_TAG}"
    echo "Target: ${TARGET}"
    echo "Platforms: ${PLATFORMS}"
    echo "Registry: ${REGISTRY}"
    echo "Dockerfile Hash: ${DOCKERFILE_HASH}"
    if [[ -n "$BRANCH_SUFFIX" ]]; then
        echo "Branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    fi
    echo "========================================="
    echo ""

    # Build and push for multiple architectures
    echo "Building and pushing multi-architecture image..."
    docker buildx build \
        --target "${TARGET}" \
        --platform "${PLATFORMS}" \
        --tag "${LATEST_TAG}" \
        --tag "${HASH_TAG}" \
        --push \
        .

    echo ""
    echo "✓ Successfully built and pushed ${LATEST_TAG}"
    echo "✓ Also tagged as ${HASH_TAG}"
    echo "✓ Available for platforms: ${PLATFORMS}"

    return 0
}

# Show usage information
show_usage() {
    cat << EOF
Docker Utilities - Container image management tools

Usage: docker-utils.sh <command> [args]

Commands:
  hash [path] [length]     Calculate Dockerfile hash
                           - path: Path to Dockerfile (default: ./Dockerfile)
                           - length: Hash length (default: 12, use 64 for full)

  pull-or-build           Pull or build Docker image with intelligent caching
                           Uses environment variables for configuration:
                           - IMAGE_NAME: Image name (default: auto-detected)
                           - REGISTRY: Registry URL (default: ghcr.io/<username>)
                           - TARGET: Build target (default: devcontainer)
                           - PLATFORM: Platform (default: auto-detected)
                           - PULL_RETRIES: Number of pull retries (default: 3)

  push [target]           Push Docker image to registry
                           Arguments:
                           - target: Build target (optional, default: devcontainer)
                           Environment variables:
                           - IMAGE_NAME: Image name (default: auto-detected)
                           - REGISTRY: Registry URL (default: ghcr.io/<username>)

  image-exists [target]   Check if image exists in registry
                           Arguments:
                           - target: Build target (optional, default: devcontainer)
                           Environment variables:
                           - IMAGE_NAME: Image name (default: auto-detected)
                           - REGISTRY: Registry URL (default: ghcr.io/<username>)
                           Returns exit code 0 if exists, 1 if not found

  build-and-push [target] Build and push multi-architecture image
                           Arguments:
                           - target: Build target (optional, default: devcontainer)
                           Environment variables:
                           - IMAGE_NAME: Image name (default: auto-detected)
                           - REGISTRY: Registry URL (default: ghcr.io/<username>)
                           - PLATFORMS: Platforms (default: linux/amd64,linux/arm64)

  repo-name               Detect repository name from git remote

  github-user             Detect GitHub username from git remote

  branch-suffix           Get branch suffix for Docker tags

  help                    Show this help message

Examples:
  # Calculate Dockerfile hash
  docker-utils.sh hash

  # Pull or build image
  docker-utils.sh pull-or-build

  # Pull or build CI target
  TARGET=ci docker-utils.sh pull-or-build

  # Push devcontainer image to registry
  docker-utils.sh push

  # Push CI image to registry
  docker-utils.sh push ci

  # Check if CI image exists in registry
  docker-utils.sh image-exists ci && echo "exists" || echo "not found"

  # Build and push devcontainer image
  docker-utils.sh build-and-push

  # Build and push CI image
  docker-utils.sh build-and-push ci

  # Build CI image for custom platforms
  PLATFORMS=linux/amd64 docker-utils.sh build-and-push ci

  # Get repo information
  docker-utils.sh repo-name
  docker-utils.sh github-user

EOF
}

# Command dispatcher - only runs when script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    case "${1:-}" in
        hash)
            calculate_dockerfile_hash "${2:-./Dockerfile}" "${3:-12}"
            ;;
        pull-or-build)
            pull_or_build
            ;;
        push)
            push_image "$2"
            ;;
        image-exists)
            image_exists "$2"
            ;;
        build-and-push)
            build_and_push "$2"
            ;;
        repo-name)
            detect_repo_name
            ;;
        github-user)
            detect_github_username
            ;;
        branch-suffix)
            get_branch_suffix
            ;;
        help|--help|-h)
            show_usage
            ;;
        "")
            echo "Error: No command specified" >&2
            echo "" >&2
            show_usage
            exit 1
            ;;
        *)
            echo "Error: Unknown command: $1" >&2
            echo "" >&2
            show_usage
            exit 1
            ;;
    esac
fi
