# Essence Burn — checkpoint per riprendere in locale

Questo è un checkpoint incompleto richiesto dall'utente il 23 ottobre 2026. La PR resta draft. Il codice comprende tutte le modifiche non pubblicate, incluse le correzioni dell'attore ancora da completare e verificare. Non ripartire dalla vecchia versione pubblicata di PR10032.

## Ripristino in una nuova directory

```bash
git clone --branch engine-source/fra-stingerquill-21 https://github.com/keloide/phase.git phase-essence-checkpoint
cd phase-essence-checkpoint
checkpoint_root="$PWD/checkpoint/essence-burn"
restore_root="$(dirname "$PWD")/essence-checkpoint-state"
mkdir -p "$restore_root"
(cd "$checkpoint_root" && sha256sum -c SHA256SUMS)
cat "$checkpoint_root"/continuity.tar.gz.part* > "$restore_root/continuity.tar.gz"
(cd "$restore_root" && sha256sum -c "$checkpoint_root/continuity-archive.sha256")
tar -xzf "$restore_root/continuity.tar.gz" -C "$restore_root"
```

La cartella separata contiene il run originale completo `engine-implementer-runs/fra-stingerquill`, compresa la storia append-only `phase-fit` fino a 778; tutti i piani/review conservati, i tentativi precedenti, il handoff originario, i log e tutti i 20 ZIP originali degli ultimi test remoti. `archive-members.json` elenca ogni file. Non sovrascrivere una cronologia locale esistente: confrontarla prima con questa copia.

## Ripristino del corpus completo

Tutti i 3.464 file del manifesto sono stati verificati con SHA256 prima del checkpoint: 3.456 nel payload, 6 già tracciati nei sorgenti, 2 in `pinned-extra`. Comprende 338 set, 3.113 deck, SetList, AtomicCards e regole; snapshot `5.3.0+20261013`, `GATED_SETS_AS_OF=2026-10-14`. Nessun input mancante è sostituito da un download aggiornato.

Il payload è conservato immutabilmente in un antenato di questa PR, evitando una seconda copia di 458 MB:

```bash
git archive 92a04a29034040362949ccae9203610d8e6c163e payload | tar -xf - -C "$restore_root"
cat "$restore_root"/payload/mtgjson-cr.tar.gz.part-* > "$restore_root/mtgjson-cr.tar.gz"
printf '%s  %s\n' 4a8802b1987118eef1c8a15c890f1444ac5a86e549ce0c22eb860c9248a822c2 "$restore_root/mtgjson-cr.tar.gz" | sha256sum -c -
tar -xzf "$restore_root/mtgjson-cr.tar.gz" -C "$PWD"
cp -a "$checkpoint_root/pinned-extra/." "$PWD/"
python3 - <<'PY'
import hashlib, json, pathlib
root = pathlib.Path.cwd()
manifest = json.loads((root/'checkpoint/essence-burn/pinned-input-manifest.json').read_text())
for entry in manifest['files']:
    path = root / entry['path']
    assert path.stat().st_size == entry['bytes'], entry['path']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == entry['sha256'], entry['path']
print('3464 input verificati')
PY
```

Questi comandi sono per il clone nuovo sopra. Il payload e `pinned-extra` ripristinano file ignorati da Git; non applicarli sopra altri input locali che si vogliono conservare. `payload-info.json`, `payload-SHA256SUMS` e il manifesto completo mantengono hash, dimensioni e provenienza.

## Stato effettivo del codice

- `a577dd5a9c757fe10d011d44ded92578b0db7c33`: salvataggio esatto delle dieci modifiche WIP dell'attore, sopra `3b4a05a93a79d5adec3ad5785c1ee5d7ba104070`. Include il test combinato U1×U3 a quattro righe. Precommit ordinario passato; nessuna accettazione dell'implementazione.
- `a4152edf586e7bd353fd88b73414466350f66c0f`: merge storico con strategia `ours`, albero identico al precedente. Conserva gli esperimenti parser/proof e il payload. NON integra il loro albero parser-only nel codice dell'attore.
- `ff218687de3a589d50442836f8cb75b0651ccdde`: candidato parser-only misurato, distinto dal codice corrente. Base fase3: `3a6f43de2baf86794975743ee67c3019ec705919`.
- Leggere `plans-and-reviews/essence-maintainer-fix-plan-9.md` interamente, insieme a plan8/review8 e alla storia completa. Il review9 completo NON esiste. Il piano corregge il finding sulla provenienza own-source/default/equality; gli otto finding originari dell'attore non sono formalmente chiusi. La patch originale resta nel run come `actor-fix1-preplan7-20261020.patch`.
- Nessuna sincronizzazione upstream finale, base congelata o verifica/recensione finale dell'HEAD attuale. PR10032 non è stata aggiornata. Il prossimo sviluppatore può continuare e scegliere gli strumenti disponibili in locale; le vecchie limitazioni di quota della sessione cloud sono evidenza storica, non un requisito di attesa per il PC locale.

## Risultati conservati, con i loro limiti

Questi risultati appartengono al candidato parser-only `ff218687...`, NON certificano il WIP attuale:

- Generazione `39737072941`: cinque export completi identici, copertura 31958/35923; snapshot 399+38 passati. Workflow complessivo rosso per lettura popolazioni senza `rg`; lettura poi recuperata dai log originali. Drift generato interamente conservato e classificato.
- Native `39824691926`: due configurazioni Clippy strict passate; nextest 40716 passati, 0 falliti, 52 ignorati di default; doctest 0 eseguiti, 7 ignorati. Il test full-DB parity è fallito (23/27 voci stale, 24 unexplained); nessuna dicitura verde per quel test.
- Completamento `39846865849`: bindings, validazione 35923 carte, fixture canonica 5911, AI 30 partite passati; warning Affinity conservato.
- Controllo base `39852124533`: lo stesso test full-DB fallisce già su `3a6f43de...`, con identici conteggi, insiemi stampati e assert. I 24 identificativi non sono stati emessi prima dell'assert: non si sostiene che siano stati confrontati singolarmente.

Tutti i 20 ZIP normali sono dentro `recovery/phase3-remote-case756/artifacts`; le copie estratte duplicate sono escluse. Non dipendere dalla scadenza degli artifact GitHub Actions. Restano conservati anche i workflow falliti e il download vuoto del log native, con il limite di trasporto documentato.

L'host cloud aveva limite cgroup 8 GiB e zero swap: niente nuove build pesanti per questo checkpoint. Le verifiche finali vanno eseguite sul codice finale, scegliendo risorse e comandi dopo aver letto le istruzioni repository e verificato i contratti CLI. Il bypass pre-push era esclusivamente il trasporto fork di questo checkpoint, con hook pesanti incompatibili con quell'host; non costituisce un PASS.

## Storia precedente

L'archivio completo precedente (~2,06 GB), con prove storiche aggiuntive, resta disponibile nel fork all'immutabile `64a65d7d83a305b1924e40c826a54b7fd2cccdb5`:

- [README storico](https://github.com/keloide/phase/blob/64a65d7d83a305b1924e40c826a54b7fd2cccdb5/README.md)
- [Archivio storico](https://github.com/keloide/phase/archive/64a65d7d83a305b1924e40c826a54b7fd2cccdb5.zip)

È una fonte storica complementare. Per il corpus ottobre13 e lo stato corrente usare questo checkpoint. `previous-pr13.json` conserva la descrizione precedente senza perderne i riferimenti. I documenti allegati sono contesto e storia; l'ultima richiesta dell'utente è la pubblicazione di questo checkpoint, non l'accettazione o la conclusione del lavoro.
