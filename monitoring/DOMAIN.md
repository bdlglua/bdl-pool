# Domaine du pool

Le minage utilise `bdl-pool.com` ; l’API utilise `api.bdl-pool.com`.
Les ports OpenPort et la clé publique Noise restent ceux déjà installés.
Le site continue à être hébergé sur GitHub Pages.

## DNS chez Northwest

Ajouter les enregistrements suivants, avec le TTL proposé :

| Type | Host | Value |
| --- | --- | --- |
| A | `@` | `193.26.115.30` |
| CNAME | `api` | `openport.pw.` |
| CNAME | `_acme-challenge.api` | `_acme-challenge.bdl-pool.duckdns.org.` |

L’adresse IPv4 est celle d’OpenPort, vérifiée lors de la configuration.
Northwest demande le point final dans les valeurs CNAME.
Si le fournisseur change d’IPv4, l’enregistrement A du pool devra être actualisé.
Le CNAME de l’API suit automatiquement le nom du relais.

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

Les adresses du site sont publiées après cette vérification.
