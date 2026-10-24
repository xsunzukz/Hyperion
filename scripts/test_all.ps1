# Mystic Hyperion Validation Suite

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Running Mystic Hyperion Validation Suite" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

# 1. Frontend Type Check & Build
Write-Host "`n[1/5] Building Frontend (TypeScript & Vite)..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Frontend build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Frontend build passed." -ForegroundColor Green

# 2. Rust Formatting Check
Write-Host "`n[2/5] Checking Rust formatting (cargo fmt)..." -ForegroundColor Yellow
cd src-tauri
cargo fmt --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "cargo fmt check failed!" -ForegroundColor Red
    cd ..
    exit 1
}
Write-Host "Formatting check passed." -ForegroundColor Green

# 3. Clippy Lint Verification
Write-Host "`n[3/5] Running Clippy linter (cargo clippy)..." -ForegroundColor Yellow
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "Clippy check failed!" -ForegroundColor Red
    cd ..
    exit 1
}
Write-Host "Clippy linter passed." -ForegroundColor Green

# 4. Native Rust Unit Tests
Write-Host "`n[4/5] Running Rust unit & integration tests (cargo test)..." -ForegroundColor Yellow
cargo test
if ($LASTEXITCODE -ne 0) {
    Write-Host "Tests failed!" -ForegroundColor Red
    cd ..
    exit 1
}
Write-Host "All tests passed." -ForegroundColor Green

# 5. Build Desktop Binary
Write-Host "`n[5/5] Building native executable (cargo build)..." -ForegroundColor Yellow
cargo build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Cargo build failed!" -ForegroundColor Red
    cd ..
    exit 1
}
cd ..
Write-Host "Native desktop build passed." -ForegroundColor Green

Write-Host "`n==========================================" -ForegroundColor Cyan
Write-Host "Validation Suite Passed: 100% OK" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Cyan
