#!/bin/bash
set -e

VERSION="1.0.0"
echo "VED v${VERSION} Release Checklist"
echo "================================"

check_item() {
    if [ $1 -eq 0 ]; then
        echo "  ✓ $2"
    else
        echo "  ✗ $2"
        exit 1
    fi
}

echo ""
echo "1. Build Tests"
cargo build --release 2>/dev/null
check_item $? "Release build compiles"

echo ""
echo "2. Unit Tests"
cargo test --all-features 2>/dev/null | grep -q "test result: ok"
check_item $? "All tests pass"

echo ""
echo "3. Example Builds"
cargo run --quiet -- build examples/hello.ved --target web --out /tmp/test-hello 2>/dev/null
check_item $? "Hello World builds"

cargo run --quiet -- build examples/counter.ved --target web --out /tmp/test-counter 2>/dev/null
check_item $? "Counter builds"

echo ""
echo "4. Binary Size Check"
BIN_SIZE=$(stat -f%z target/release/vedc 2>/dev/null || stat -c%s target/release/vedc)
if [ $BIN_SIZE -lt 25000000 ]; then
    check_item 0 "Binary size OK ($(($BIN_SIZE / 1024 / 1024))MB < 25MB)"
else
    check_item 1 "Binary too large ($(($BIN_SIZE / 1024 / 1024))MB)"
fi

echo ""
echo "5. Documentation"
check_item $([ -f README.md ] && echo 0 || echo 1) "README.md exists"
check_item $([ -f CHANGELOG.md ] && echo 0 || echo 1) "CHANGELOG.md exists"
check_item $([ -d examples ] && echo 0 || echo 1) "examples/ exists"

echo ""
echo "================================"
echo "✓ All checks passed! Ready to release v${VERSION}"
echo ""
echo "Next steps:"
echo "1. git tag v${VERSION}"
echo "2. git push origin v${VERSION}"
echo "3. GitHub Actions will build and release"
