# Domaine du pool

Le site utilise `https://bdl-pool.com/`, le minage `stratum.bdl-pool.com`
et l’API `api.bdl-pool.com`.
Les ports OpenPort et la clé publique Noise restent ceux déjà installés.
Le site continue à être hébergé sur GitHub Pages.

## DNS chez Northwest

Ajouter les enregistrements suivants, avec le TTL proposé :

| Type | Host | Value |
| --- | --- | --- |
| CNAME | `stratum` | `openport.pw.` |
| CNAME | `api` | `openport.pw.` |
| CNAME | `_acme-challenge.api` | `_acme-challenge.bdl-pool.duckdns.org.` |

Northwest demande le point final dans les valeurs CNAME.
Les CNAME de Stratum et de l’API suivent automatiquement le nom du relais.
Le domaine principal est réservé au site GitHub Pages.

Conserver les autres enregistrements, notamment MX, SPF, DKIM, DMARC et les TXT
`_acme-challenge` de Northwest. Ne pas ajouter de CNAME sur ce dernier nom :
la délégation de notre API se trouve sur **`_acme-challenge.api`**.
La délégation permet de réutiliser le jeton DuckDNS déjà enregistré sur le
serveur, sans communiquer de nouveaux identifiants DNS.

## Configurer HTTPS

Une fois les trois enregistrements visibles, lancer avec le compte `bdl` :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
sudo python3 /home/bdl/bdl-pool-site/monitoring/custom-domain.py
```

Le script valide les réponses DNS autoritaires, les profils OpenPort et l’API
existante. Il installe un hook root, obtient le certificat via DNS-01 et teste
son renouvellement. Il configure ensuite Caddy et actualise les adresses
Stratum dans l’API. Si l’activation échoue, les configurations précédentes sont
restaurées. Les sauvegardes et journaux restent privés sur le serveur.

Les certificats de l’ancien domaine et du nouveau sous-domaine sont stockés
séparément. L’ancienne API et son renouvellement restent accessibles pendant la
bascule. Le timer Certbot renouvelle aussi le nouveau certificat automatiquement.
Le jeton dans `/etc/bdl-pool-openport/duckdns.json` et les enregistrements de
délégation doivent rester valides pour ces renouvellements.

Bitcoin Core, CKPool, les tunnels et les réglages du routeur ne sont pas modifiés.
Seuls Caddy est rechargé et le service API est redémarré.

Après succès, faire vérifier depuis Internet :

```text
https://api.bdl-pool.com:36018/api/v1/health
```

## Site GitHub Pages sur le domaine principal

Le fichier `CNAME` à la racine du dépôt contient `bdl-pool.com`. La publication
GitHub Pages utilise la branche `main`, dossier `/`. Vérifier que Settings →
Pages → Custom domain contient ce nom avant de modifier le DNS du site.

Chez Northwest, remplacer l’ancien A `@ → 193.26.115.30` par les quatre A de
GitHub Pages, et ajouter le CNAME `www` :

| Type | Host | Value |
| --- | --- | --- |
| A | `@` | `185.199.108.153` |
| A | `@` | `185.199.109.153` |
| A | `@` | `185.199.110.153` |
| A | `@` | `185.199.111.153` |
| CNAME | `www` | `bdlglua.github.io.` |

Conserver les CNAME `stratum`, `api` et `_acme-challenge.api` ainsi que les
enregistrements de messagerie. Une fois le certificat GitHub Pages disponible,
activer Enforce HTTPS dans les paramètres Pages.

Les anciens liens `bdlglua.github.io/bdl-pool/` redirigent vers le nouveau
domaine. Les liens internes et les pages françaises et anglaises utilisent
les mêmes chemins à partir de la racine du nouveau domaine.

Documentation : [domaine personnalisé GitHub Pages](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site).
