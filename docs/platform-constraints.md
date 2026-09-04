# Contraintes de plateforme

Mêmes règles que les autres fichiers de `docs/` : une contrainte non évidente se documente ici et se
vérifie par un test au nom explicite, jamais par un commentaire dans le code.

| Contrainte | Conséquence pour Quay | Test qui la vérifie |
|---|---|---|
| Le trousseau macOS lie l'autorisation d'un secret au binaire qui l'a créé. **Recompiler change la signature du binaire et le système redemande l'autorisation**, par un dialogue graphique | Un `quay push` lancé depuis un shell non interactif se bloque indéfiniment, sans message. La lecture du trousseau est donc bornée dans le temps et rend une erreur qui nomme la cause et l'issue | `work_that_never_answers_gives_up_instead_of_blocking_the_process` |
| Un jeton fourni explicitement par l'environnement doit primer sur le trousseau | Une session de développement ou de CI ne touche jamais au magasin d'identifiants, donc ne peut pas se bloquer dessus | `an_absent_environment_variable_yields_no_token` |
| Le magasin d'identifiants du système est absent d'une CI sans session graphique | Les tests qui le touchent portent `#[ignore]` avec la raison en toutes lettres, et se lancent à la main sur une machine de développement | les trois tests `keychain::tests::*`, marqués `#[ignore]` |
| La coque desktop embarque une webview, qui se lie à des bibliothèques système sur Linux | Les jobs Rust de la CI installent `libwebkit2gtk`, `libgtk-3`, `librsvg2`, `libayatana-appindicator3` et `libxdo` avant de compiler, sinon l'échec survient à l'édition de liens et non à la compilation. **Non vérifié depuis cette machine**, qui est sous macOS | aucun ; à confirmer au premier passage de la CI |
| Tauri tolère l'absence du bundle frontend à la compilation | Les jobs Rust n'ont pas besoin de `npm run build`. Vérifié en retirant `apps/desktop/dist` puis en recompilant | vérifié à la main, non automatisé |
| Tauri sert `devUrl` au lieu du bundle embarqué tant que la fonctionnalité `custom-protocol` du crate `tauri` n'est pas activée. La CLI Tauri l'active pour les builds de release ; sans elle, personne ne le fait | La fenêtre s'ouvrait **entièrement blanche**, sans erreur ni journal. La fonctionnalité est activée par défaut dans `quay-app` | `the_window_serves_the_frontend_it_carries_rather_than_a_development_server` |
| Tauri 2 refuse les commandes de ses greffons de cœur, dont `listen`, tant qu'une capacité ne les accorde pas à la fenêtre | Les événements du §5.4 n'arrivaient jamais au webview alors que les commandes propres à l'application fonctionnaient. `capabilities/default.json` accorde `core:default` à la fenêtre `main` | non automatisé ; la fenêtre affiche désormais l'échec d'abonnement au lieu de l'avaler |
| `tracing_subscriber::fmt()` écrit sur **stdout**, pas sur stderr | Le harnais de mesure du démarrage à froid ne capturait rien tant qu'il ne lisait que stderr | `a_plain_line_yields_its_elapsed_milliseconds` |
