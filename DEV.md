# Ban desktop tro ve Lab

Profile lab tro ve https://fkit-dev.joinnow.vn va https://fdict-dev.joinnow.vn. Chon luc build bang bien FDICT_BUILD_ENV=lab.

## Chay (PowerShell)
    $env:FDICT_BUILD_ENV="lab"; yarn tauri dev

Build thu (khong phat hanh):
    $env:FDICT_BUILD_ENV="lab"; cargo build --manifest-path src-tauri/Cargo.toml

Ban lab kiem tra cap nhat o https://fdict-dev.joinnow.vn/api/desktop/latest, khong dung kenh cap nhat cua khach.
Khong phat hanh ban lab len GitHub release va khong dang len trang tai cua khach.
Can fdict-web-app (8001) va fkit-landing (8000) dang chay voi .env.lab.
