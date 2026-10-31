# AVM Procurement Council — pornire rapidă

Contract Rust pentru SDK AVM 0.3.0: buget comun, 2–5 aprobatori, comenzi cu până
la 4 etape, voturi legate de livrarea exactă, întârziere înainte de plată,
pauză de urgență și decontare prin callback. Include contractul destinatar
pentru teste, surse, artefacte WASM/ABI/descriptor și rezultatele verificărilor.

## Verificare

Pe Linux compatibil cu SDK-ul, ai nevoie de Python 3.11+, Bash și Rust 1.93.0
cu targetul wasm32-unknown-unknown, rustfmt și clippy. Din directorul proiectului:

```bash
python3 scripts/setup_sdk.py
bash scripts/check.sh
```

Instalatorul descarcă exclusiv versiunea oficială fixată în proiect, verifică
SHA-256 și folosește verificatorul oficial pentru conținut. Rulează standuri
locale noi. Nu folosește conturi sau fonduri reale.

Alternativ, pentru un SDK 0.3.0 deja verificat:

```bash
bash scripts/check.sh /cale/absoluta/avm-sdk-linux-x86_64
```

Doar testele Rust, fără descărcarea pachetului binar SDK:

```bash
cargo +1.93.0 test --workspace --locked --offline
```

## Ce dovedește livrarea

- 57 teste proprii și 9 teste moștenite din SDK, toate trecute.
- Compilare cu builderul oficial, care compară două compilări pentru fiecare contract.
- Scenariu AVM real: 3 plăți pe etape, o respingere cu rambursare, reîncercare,
  revizuire a livrării, voturi vechi respinse, pauză/reluare, anulare și expirare.
- Scenariu la limită: 8 comenzi cu câte 4 etape, 64 încercări de plată și blocarea
  celei de-a 65-a înainte de emitere.
- Replay identic pentru ambele scenarii.

Vezi `README.md`, `docs/SPEC.md`, `docs/SDK-INTEGRATION.md` și
`evidence/qualification.json`. Documentația principală este în engleză pentru
review și eventuală includere în SDK.

Este un exemplu funcțional pentru suita SDK, cu limite intenționate: 8 comenzi
pe durata contractului, 64 încercări, fără migrare, retragere generală sau
recuperare automată pentru un callback pierdut. Nu reprezintă audit ori validare
pentru custodia fondurilor reale. Workflow-ul `.github/workflows/qualification.yml` a trecut și pe GitHub, pe commitul `e5e59d0`: rularea 41153010473.

Arhiva de livrare conține și `git-history.bundle`, un export al istoricului Git local.
