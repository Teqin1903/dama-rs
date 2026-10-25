# Kurulum (takımdaki herkes için)

## 1. Araçlar (CachyOS / Arch)
```
sudo pacman -S rustup nodejs          # rustup: Rust sürüm yöneticisi; nodejs: testdata'yı yeniden üretmek için
rustup default stable                 # derleyici + cargo + clippy + rustfmt
rustup component add rust-analyzer    # Claude Code'un LSP eklentisinin konuşacağı dil sunucusu
command -v rust-analyzer              # bir yol yazmalı; boşsa: sudo pacman -S rust-analyzer
```

## 2. Depoyu ilk kez kuran kişi
```
cd dama-rs-starter                     # bu klasör deponun kökü olacak
git init
git config core.hooksPath .githooks    # commit mesajındaki Claude satırlarını silen hook'u açar
git add -A
git commit -m "Start from the JS prototype and testdata"
git branch -M main                     # dalın adını main yapar (git'in varsayılanı master olabilir)
git remote add origin <github-deposunun-adresi>
git push -u origin main
```

## 3. Diğer takım arkadaşları
```
git clone <github-deposunun-adresi> dama-rs
cd dama-rs
git config core.hooksPath .githooks    # hook ayarı git'e kaydedilmez, her klonda bir kez yapılır
```

## 4. Claude Code (proje klasöründe `claude` ile başlat)
```
/plugin install rust-analyzer-lsp@claude-plugins-official
/output-style learning
```
- `CLAUDE.md` ve `.claude/settings.json` depoda olduğu için herkesin Claude'u aynı kurallarla çalışır:
  Claude satırı commit'lere eklenmez, `reference/` ve `testdata/` Claude'a karşı yazmaya kapalıdır,
  `cargo build / check / test / clippy / fmt` izin sormadan çalışır.
- Kendine özel notların için `CLAUDE.local.md` aç; `.gitignore` onu depoya göndermez.
