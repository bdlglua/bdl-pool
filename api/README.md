# API BDL Pool

API publique en lecture seule, hébergée sur le serveur du pool. L’interface et la
documentation restent sur GitHub Pages. Aucun worker, share ou réglage n’est créé
par une requête API.

## Installation sur le serveur

Le suivi automatique doit déjà être installé. Les ports TCP 80 et 443 doivent
être redirigés vers le serveur du pool, et `bdl-pool.duckdns.org` doit pointer vers
son adresse publique. Le compte d’exécution est `bdl`, sans sudo devant le script :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
bash /home/bdl/bdl-pool-site/api/install.sh
```

Le script installe Caddy depuis les paquets Debian, crée `bdl-pool-api.service`
et ajoute un fragment Caddy après validation. Une sauvegarde de Caddyfile est
créée ; un candidat invalide est restauré avant tout rechargement. Un service
Nginx/Apache actif ou un port déjà pris par un autre service interrompt le script.
Les ports Stratum, Bitcoin Core et le service CKPool ne sont pas modifiés.

Python écoute uniquement sur `127.0.0.1:8081`. Caddy termine le HTTPS sur 443,
renouvelle le certificat automatiquement et transmet `/api/v1/*` à Python.
Le service Python voit uniquement son code, sa configuration publique et les
statistiques en lecture seule ; son accès au reste du dossier personnel est masqué.
Le port 8081 n’a pas à être redirigé sur le routeur.

## Endpoints

Base : `https://bdl-pool.duckdns.org/api/v1`

| GET | Réponse |
| --- | --- |
| `/health` | État du processus API, indépendant de Bitcoin/CKPool |
| `/pool` | Statistiques globales, état Bitcoin, dates et contrôles |
| `/history` | Dernières mesures de hashrate disponibles |
| `/stratum` | Adresse, ports, URL V1/V2 et clé publique V2 |
| `/miners/{address}` | Hashrates, shares et meilleure difficulté d’une adresse BTC |
| `/miners/{address}/workers` | Workers de cette adresse ; `limit=50&offset=0` |
| `/miners/{address}/workers/{worker}` | Détail d’un worker, nom complet ou suffixe |
| `/openapi.json` | Contrat OpenAPI 3.1 |

Exemple avec une adresse utilisée pour miner :

```bash
curl https://bdl-pool.duckdns.org/api/v1/pool
curl 'https://bdl-pool.duckdns.org/api/v1/miners/VOTRE_ADRESSE_BTC/workers?limit=50&offset=0'
```

`address` est une adresse Bitcoin mainnet, sans suffixe de worker. Sa syntaxe est
contrôlée ; cette validation n’est pas une vérification de somme de contrôle.
`worker` peut être le nom complet renvoyé par la liste, ou son suffixe après `.`
ou `_`. Si deux noms ont le même suffixe, utiliser le nom complet. Aucun endpoint
ne fournit de répertoire de toutes les adresses du pool.

## Données et dates

Les informations globales proviennent du relevé public déjà publié par le suivi,
actualisé toutes les cinq minutes. `updatedAt` reste la date de la mesure ;
`ageSeconds` et `stale` indiquent son ancienneté. Un relevé de plus de quinze
minutes, un nœud conservé après erreur RPC ou un contrôle Stratum sans job ne
peut pas produire `readyAtLastCheck: true`. Ce champ concerne le dernier contrôle
**Stratum V1**, pas la disponibilité présente de V2.

Les statistiques individuelles sont lues dans `logs/users/{address}`. Dans la
révision CKPool installée, les workers sont inclus dans la propriété `worker` de
ce même fichier ; il n’y a pas de dossier `workers/` à parcourir. Le fichier n’est
créé qu’après une première share enregistrée. Une adresse encore inconnue renvoie
404, notamment tant que Bitcoin est en synchronisation. Un fichier de plus de
trois minutes reste consultable avec `stale: true`. Cela n’indique pas que le
mineur est connecté. Les valeurs absentes restent `null`.

Les hashrates sont exprimés en H/s. `bestShare` et `bestEver` sont des difficultés.
`shares` reprend le compteur CKPool. Aucune estimation de revenu ni de paiement
n’est déduite de ces compteurs. Les IP, les mots de passe et les champs inconnus
des fichiers sources sont exclus de toutes les réponses.

GET, HEAD et OPTIONS sont autorisés ; les écritures renvoient 405. CORS public
(`*`), cache de cinq secondes et ETag. Pagination : limite 1–200, défaut 50.
400 : paramètres invalides ; 404 : adresse/worker/endpoint absent ; 503 : source
absente, invalide ou en cours d’écriture. Les chemins et requêtes des adresses
ne sont pas écrits dans les journaux de l’API ; l’accès log Caddy n’est pas activé.

## Vérifier et mettre à jour

```bash
curl -fsS https://bdl-pool.duckdns.org/api/v1/health
systemctl status bdl-pool-api caddy --no-pager
journalctl -u bdl-pool-api -n 20 --no-pager

git -C /home/bdl/bdl-pool-site pull --ff-only
sudo systemctl restart bdl-pool-api
```

Tests sans Bitcoin ni CKPool : `python3 -m unittest discover -s api -p 'test_*.py'`.
Ils utilisent des statistiques fictives dans un dossier temporaire et de vrais
appels HTTP locaux. Aucun paquet ni service système n’est installé par les tests.
