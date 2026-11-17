# E.D.I.T.H. — Windows sürümü

Rust ile yazılmış sesli asistan. Sohbet terminalde, ayarlar ayrı bir pencerede (mobil sürümle aynı tasarım).

## Nasıl alınır
`main` dalına her gönderimde GitHub Actions Windows için derler. Bitince **Releases → Son sürüm** altındaki
`edith-windows.zip` dosyasını indirip bir klasöre çıkar, `edith.exe` ile başlat (klasördeki DLL'ler yanında kalsın).

## Komutlar
- `ayarlar` — ayarlar penceresi
- `/model` — modelleri numaralı listeler, numara yazınca o sohbetin modeli olur
- `/history_chat` — eski sohbetleri listeler, numara yazınca o sohbete girer
- `/yeni` — yeni sohbet · `/yardim` · `çıkış`
- Sesle: "hey edit" / "edit"

Ayarlar ve sohbetler `%APPDATA%\EDITH` klasöründe tutulur; API anahtarları repoya girmez.
