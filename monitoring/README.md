# Suivi automatique — serveur

Le collecteur fonctionne avec Python 3 standard, Git, SSH, `flock` et systemd.
Il s'exécute sous `bdl`, comme `bitcoin-cli`, et lit les fichiers CKPool sans
modifier Bitcoin Core, CKPool, leur configuration ou les ports du routeur.

## Installation

Créer une clé SSH dédiée `~/.ssh/bdl-pool-stats`, puis ajouter sa partie publique
comme **Deploy key** du dépôt `bdlglua/bdl-pool` avec **Allow write access**.
Ne jamais copier la clé privée sur GitHub ou dans le chat. Une deploy key en
écriture peut modifier tout ce dépôt ; le programme ne pousse que sur `stats`.

Avec le compte `bdl`, sans lancer l'ensemble avec sudo :

```bash
git clone https://github.com/bdlglua/bdl-pool.git /home/bdl/bdl-pool-site
bash /home/bdl/bdl-pool-site/monitoring/install.sh
```

Le script vérifie l'accès SSH (la première connexion peut demander de vérifier
l'empreinte officielle GitHub), publie un premier relevé, puis installe les unités
systemd avec sudo. Il ne demande aucun jeton GitHub ni mot de passe RPC.
La synchronisation Bitcoin peut encore être en cours ; les mesures CKPool
absentes sont alors affichées « — ».

## Fonctionnement

- Toutes les cinq minutes, le service appelle `/usr/local/bin/bitcoin-cli
  -rpcclienttimeout=15 getblockchaininfo` sous `bdl`. Le cookie RPC reste local.
- Il ne retient que la chaîne, les blocs, les en-têtes, la progression et l'état
  de synchronisation. Une panne RPC donne des champs inconnus, sans republier
  un ancien état comme s'il était récent.
- Le `logdir` est lu dans `/home/bdl/mining/ckpool/local.conf`, le fichier utilisé par le service CKPool ; s'il est relatif,
  le dossier de travail du service CKPool puis le dossier de sa configuration
  sont recherchés. Le fichier attendu est `logs/pool/pool.status` dans le cas présent.
- CKPool écrit plusieurs objets JSON successifs. Le collecteur les lit, puis
  sélectionne seulement hashrate5m (converti en H/s), Workers et bestshare.
  Les fichiers de plus de trois minutes sont ignorés. Les blocs trouvés restent
  inconnus car ce compteur n'est pas fourni de manière fiable par pool.status.
  Aucun journal brut, adresse de portefeuille, IP de mineur ou secret n'est publié.
- Après la synchronisation du nœud, un abonnement Stratum court est testé sur
  `192.168.1.2:3333`. Il ne crée aucun worker autorisé et ne soumet aucune share.
  Le pool n'est indiqué disponible que si l'abonnement réussit et qu'un job
  `mining.notify` est reçu. Certaines configurations solo n'envoient un job
  personnalisé qu'après autorisation : le site laisse alors la disponibilité
  à vérifier, même si le pool fonctionne pour ses vrais mineurs.
- Le relevé contient `source: "snapshot"`. Les points d'historique sont de vraies
  mesures, limités aux dernières 24 heures (288 points). Les données de plus de
  quinze minutes ne confirment jamais la disponibilité.
- Un clone séparé sous `~/.local/state/bdl-pool-monitor/repository` reçoit
  uniquement `status.json` et pousse sur la branche **stats**, sans force push.
  Le site lit ce JSON HTTPS via raw.githubusercontent.com (CORS public).
  Les caches GitHub peuvent ajouter plusieurs minutes de délai.
- **main** continue de servir le site via GitHub Pages. Les relevés ne provoquent
  pas de builds Pages et le collecteur ne modifie aucun fichier du site.

Les statistiques deviennent publiques, comme le site. Le timer fonctionne après
redémarrage et n'a pas besoin d'une session SSH ouverte.

## Vérifier / arrêter

```bash
systemctl list-timers bdl-pool-monitor.timer --no-pager
journalctl -u bdl-pool-monitor.service -n 30 --no-pager
sudo systemctl start bdl-pool-monitor.service
```

Pour arrêter les publications :

```bash
sudo systemctl disable --now bdl-pool-monitor.timer
```

Pour retirer aussi l'accès GitHub, supprimer la deploy key dans Settings → Deploy keys.

## Diagnostic local sans publication

```bash
python3 /home/bdl/bdl-pool-site/monitoring/collect.py
```

Cette commande imprime uniquement le relevé public filtré. Si le dossier des
journaux ne peut pas être détecté, tester avec `--pool-status /chemin/pool/pool.status`.
Ne pas publier bitcoin.conf, local.conf, ckpool.conf, le cookie RPC ou une sortie de journal brut.
