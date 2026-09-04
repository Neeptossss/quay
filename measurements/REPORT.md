# Rapport de mesures — Quay

Ce fichier est généré par `cargo run -p xtask -- report` à partir des logs bruts de `measurements/raw/`. Il ne se modifie pas à la main.

- Généré le : 2026-09-04T14:17:51.648968Z
- Machine : aarch64-macos

## Budgets du §4

`MISSING` signifie qu'aucune mesure n'existe encore pour ce budget : il est rouge parce qu'un budget non mesuré est un budget non tenu, pas parce qu'il est dépassé.

| Budget | Réf. | Borne | Statistique | Instrument | Mesure | Verdict | Log brut |
|---|---|---|---|---|---|---|---|
| `cold_start_to_first_paint` (cold start to first painted list) | §4 | ≤ 400 ms | median | tauri trace, process start to paint | — | **MISSING** | — |
| `keystroke_to_pixel` (keystroke to updated pixel) | §4 | ≤ 16 ms | p99 | frontend instrumentation, performance.now() | — | **MISSING** | — |
| `cached_pull_request_navigation` (navigation to the next pull request from cache) | §4 | ≤ 50 ms | median | frontend instrumentation, performance.now() | — | **MISSING** | — |
| `inbox_query` (filtered inbox query on SQLite) | §4 | ≤ 5 ms | p99 | xtask measure j1b, reference dataset | 0.249 ms | **MET** | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-warm.csv` |
| `diff_open_two_thousand_lines` (opening a 2000 line diff) | §4 | ≤ 120 ms | median | bench, virtualised rendering | — | **MISSING** | — |
| `resident_memory_idle` (resident memory at rest, 20 repositories synchronised) | §4 | ≤ 250 MB | max | process measurement | — | **MISSING** | — |
| `background_cpu_idle` (cpu at rest, window in the background) | §4 | ≤ 0.5 % | mean | process measurement | — | **MISSING** | — |
| `github_quota_per_hour` (GitHub quota consumed in normal operation) | §4 | ≤ 1500 req/h | max | rate governor counter | — | **MISSING** | — |
| `cache_served_navigations` (navigations served from cache in under 50 ms) | §4 | ≥ 85 % | sliding window | preload_outcome | — | **MISSING** | — |

## J1-a — point de rupture de la requête GraphQL de détail de pull request

- Mesuré le : 2026-09-04T14:14:17.385968Z
- Machine : aarch64-macos
- Log brut : `measurements/raw/j1a-2026-09-04T141417.385968Z.jsonl`

| Cible | Sélection | Fichiers | Threads | Mode | Pagination | Rép. | p50 ms | p95 ms | max ms | Octets | Coût | Nœuds | Fichiers rendus | Threads rendus | Tronqué | Erreurs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `rust-lang/rust#161795` | files~5 | 5 | 3 | single_shot | small_pages | 7 | 719 | 864 | 864 | 3288 | 1 | 241 | 5/5 | 3/3 | non | 0 |
| `rust-lang/rust#161795` | files~5 | 5 | 3 | single_shot | medium_pages | 7 | 690 | 930 | 930 | 3289 | 1 | 1351 | 5/5 | 3/3 | non | 0 |
| `rust-lang/rust#161795` | files~5 | 5 | 3 | single_shot | max_pages | 7 | 866 | 968 | 968 | 3290 | 1 | 10201 | 5/5 | 3/3 | non | 0 |
| `rust-lang/rust#161795` | files~5 | 5 | 3 | paginated | files100_threads50_comments25 | 7 | 832 | 1050 | 1050 | 3039 | 2 | 1400 | 5/5 | 3/3 | non | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | single_shot | small_pages | 7 | 752 | 945 | 945 | 13087 | 1 | 241 | 20/50 | 13/13 | oui | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | single_shot | medium_pages | 7 | 945 | 1611 | 1611 | 15935 | 1 | 1351 | 50/50 | 13/13 | non | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | single_shot | max_pages | 7 | 819 | 1025 | 1025 | 15936 | 1 | 10201 | 50/50 | 13/13 | non | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | paginated | files100_threads50_comments25 | 7 | 1022 | 1526 | 1526 | 15696 | 2 | 1400 | 50/50 | 13/13 | non | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | single_shot | small_pages | 7 | 873 | 963 | 963 | 3342 | 1 | 241 | 20/323 | 2/2 | oui | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | single_shot | medium_pages | 7 | 736 | 842 | 842 | 6244 | 1 | 1351 | 50/323 | 2/2 | oui | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | single_shot | max_pages | 7 | 867 | 1146 | 1146 | 11247 | 1 | 10201 | 100/323 | 2/2 | oui | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | paginated | files100_threads50_comments25 | 7 | 2397 | 2676 | 2676 | 32724 | 5 | 1700 | 323/323 | 2/2 | non | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | single_shot | small_pages | 7 | 719 | 1253 | 1253 | 12076 | 1 | 241 | 20/70 | 20/170 | oui | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | single_shot | medium_pages | 7 | 1160 | 1705 | 1705 | 36682 | 1 | 1351 | 50/70 | 50/170 | oui | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | single_shot | max_pages | 7 | 1423 | 2234 | 2234 | 72608 | 1 | 10201 | 70/70 | 100/170 | oui | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | paginated | files100_threads50_comments25 | 7 | 3682 | 3827 | 3827 | 119490 | 5 | 5300 | 70/70 | 170/170 | non | 0 |

## J1-b — requête d'inbox sur dataset synthétique

- Mesuré le : 2026-09-04T13:59:51.52716Z
- Machine : aarch64-macos

Le budget `inbox_query` retient le p99 le plus défavorable des quatre requêtes sur le dataset de référence, cache chaud.

| Dataset | Requête | Cache | Lignes | p50 ms | p95 ms | p99 ms | max ms | Échantillons | Log brut |
|---|---|---|---|---|---|---|---|---|---|
| 20repos-300open-0closed-5000comments | `open_across_tracked_repos` | warm | 50 | 0.031 | 0.045 | 0.062 | 0.154 | 10000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_across_tracked_repos-warm.csv` |
| 20repos-300open-0closed-5000comments | `open_across_tracked_repos` | cold | 50 | 0.085 | 0.117 | 0.130 | 0.147 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_across_tracked_repos-cold.csv` |
| 20repos-300open-0closed-5000comments | `open_with_unresolved_thread_count` | warm | 50 | 0.154 | 0.202 | 0.249 | 0.620 | 10000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-warm.csv` |
| 20repos-300open-0closed-5000comments | `open_with_unresolved_thread_count` | cold | 50 | 0.277 | 0.405 | 0.720 | 1.210 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-cold.csv` |
| 20repos-300open-0closed-5000comments | `review_requested_approximation` | warm | 50 | 0.047 | 0.062 | 0.120 | 0.619 | 10000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-review_requested_approximation-warm.csv` |
| 20repos-300open-0closed-5000comments | `review_requested_approximation` | cold | 50 | 0.184 | 1.190 | 1.665 | 3.471 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-review_requested_approximation-cold.csv` |
| 20repos-300open-0closed-5000comments | `open_filtered_by_common_title` | warm | 50 | 0.034 | 0.043 | 0.063 | 0.148 | 10000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_filtered_by_common_title-warm.csv` |
| 20repos-300open-0closed-5000comments | `open_filtered_by_common_title` | cold | 50 | 0.095 | 0.133 | 0.197 | 0.515 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_filtered_by_common_title-cold.csv` |
| 20repos-300open-0closed-5000comments | `open_filtered_by_rare_title` | warm | 0 | 0.048 | 0.067 | 0.102 | 0.308 | 10000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_filtered_by_rare_title-warm.csv` |
| 20repos-300open-0closed-5000comments | `open_filtered_by_rare_title` | cold | 0 | 0.242 | 0.357 | 0.400 | 0.509 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-300open-0closed-5000comments-open_filtered_by_rare_title-cold.csv` |
| 20repos-3000open-0closed-50000comments | `open_across_tracked_repos` | warm | 50 | 0.031 | 0.041 | 0.056 | 0.097 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_across_tracked_repos-warm.csv` |
| 20repos-3000open-0closed-50000comments | `open_across_tracked_repos` | cold | 50 | 0.083 | 0.116 | 0.141 | 0.149 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_across_tracked_repos-cold.csv` |
| 20repos-3000open-0closed-50000comments | `open_with_unresolved_thread_count` | warm | 50 | 11.927 | 12.895 | 13.340 | 20.584 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_with_unresolved_thread_count-warm.csv` |
| 20repos-3000open-0closed-50000comments | `open_with_unresolved_thread_count` | cold | 50 | 12.027 | 12.695 | 13.200 | 14.491 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_with_unresolved_thread_count-cold.csv` |
| 20repos-3000open-0closed-50000comments | `review_requested_approximation` | warm | 50 | 0.047 | 0.067 | 0.106 | 0.246 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-review_requested_approximation-warm.csv` |
| 20repos-3000open-0closed-50000comments | `review_requested_approximation` | cold | 50 | 0.142 | 0.187 | 0.213 | 0.220 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-review_requested_approximation-cold.csv` |
| 20repos-3000open-0closed-50000comments | `open_filtered_by_common_title` | warm | 50 | 0.035 | 0.050 | 0.068 | 0.220 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_filtered_by_common_title-warm.csv` |
| 20repos-3000open-0closed-50000comments | `open_filtered_by_common_title` | cold | 50 | 0.092 | 0.138 | 0.145 | 0.181 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_filtered_by_common_title-cold.csv` |
| 20repos-3000open-0closed-50000comments | `open_filtered_by_rare_title` | warm | 0 | 2.125 | 2.955 | 7.144 | 13.525 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_filtered_by_rare_title-warm.csv` |
| 20repos-3000open-0closed-50000comments | `open_filtered_by_rare_title` | cold | 0 | 2.180 | 2.660 | 2.890 | 3.651 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-3000open-0closed-50000comments-open_filtered_by_rare_title-cold.csv` |
| 20repos-30000open-0closed-500000comments | `open_across_tracked_repos` | warm | 50 | 0.031 | 0.039 | 0.066 | 0.227 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_across_tracked_repos-warm.csv` |
| 20repos-30000open-0closed-500000comments | `open_across_tracked_repos` | cold | 50 | 0.093 | 0.129 | 0.136 | 0.142 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_across_tracked_repos-cold.csv` |
| 20repos-30000open-0closed-500000comments | `open_with_unresolved_thread_count` | warm | 50 | 192.629 | 226.503 | 295.617 | 638.681 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_with_unresolved_thread_count-warm.csv` |
| 20repos-30000open-0closed-500000comments | `open_with_unresolved_thread_count` | cold | 50 | 190.500 | 200.178 | 208.362 | 217.698 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_with_unresolved_thread_count-cold.csv` |
| 20repos-30000open-0closed-500000comments | `review_requested_approximation` | warm | 50 | 0.045 | 0.057 | 0.071 | 0.176 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-review_requested_approximation-warm.csv` |
| 20repos-30000open-0closed-500000comments | `review_requested_approximation` | cold | 50 | 0.132 | 0.171 | 0.222 | 0.229 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-review_requested_approximation-cold.csv` |
| 20repos-30000open-0closed-500000comments | `open_filtered_by_common_title` | warm | 50 | 0.034 | 0.044 | 0.060 | 0.206 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_filtered_by_common_title-warm.csv` |
| 20repos-30000open-0closed-500000comments | `open_filtered_by_common_title` | cold | 50 | 0.084 | 0.115 | 0.137 | 0.140 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_filtered_by_common_title-cold.csv` |
| 20repos-30000open-0closed-500000comments | `open_filtered_by_rare_title` | warm | 0 | 19.883 | 21.544 | 28.237 | 68.610 | 2000 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_filtered_by_rare_title-warm.csv` |
| 20repos-30000open-0closed-500000comments | `open_filtered_by_rare_title` | cold | 0 | 19.845 | 20.637 | 25.898 | 65.212 | 100 | `measurements/raw/j1b-2026-09-04T135951.52716Z-20repos-30000open-0closed-500000comments-open_filtered_by_rare_title-cold.csv` |

### Plans d'exécution SQLite

- `open_across_tracked_repos` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `open_with_unresolved_thread_count` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?) ; CORRELATED SCALAR SUBQUERY 1 ; BLOOM FILTER ON t (pr_id=? AND is_resolved=?) ; SEARCH t USING AUTOMATIC PARTIAL COVERING INDEX (pr_id=? AND is_resolved=?)
- `review_requested_approximation` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `open_filtered_by_common_title` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `open_filtered_by_rare_title` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)

