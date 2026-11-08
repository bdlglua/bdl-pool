# Essai de Bore

Bore transmet un port TCP à travers le relais public `bore.pub`. Ce script
installe le binaire officiel **0.6.0** sous
`~/.local/share/bdl-pool-bore/0.6.0/`, après vérification SHA-256.
Linux ARM64 et x86-64 sont pris en charge, sans sudo ni compilation.

## Tester le transport

Avec le compte `bdl` :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
python3 /home/bdl/bdl-pool-site/monitoring/try-bore.py test
```

Le script crée un serveur HTTP temporaire sur une adresse de boucle locale et
affiche une **URL de test**. Garder ce terminal ouvert et transmettre cette URL
pour un contrôle depuis Internet. Elle ne sert qu'une réponse fixe
`BDL-BORE-OK`, aucun fichier local. Le test s'arrête après une requête valide,
après dix minutes ou avec Ctrl+C. Bitcoin Core et CKPool restent inchangés.
Le test fonctionne pendant la synchronisation Bitcoin, même sans CKPool actif.

## Essayer Stratum

Lorsque CKPool écoute réellement, lancer dans deux terminaux distincts :

```bash
python3 /home/bdl/bdl-pool-site/monitoring/try-bore.py v1
```

```bash
python3 /home/bdl/bdl-pool-site/monitoring/try-bore.py v2
```

Chaque essai reste limité à dix minutes. Utiliser l'adresse et le port affichés,
pas les ports 3333/3336 du serveur local. La clé publique Noise V2 reste celle
du pool. `--remote-port PORT` permet de redemander un port, mais échoue s'il
n'est pas libre ou autorisé par le relais. Aucun port n'est réservé.

La connexion de contrôle de Bore utilise TCP **7835 en sortie**. Aucune nouvelle
redirection Freebox n'est nécessaire. Une fermeture du script coupe le tunnel ;
aucun service permanent n'est installé. Pour supprimer le binaire :
`rm -r -- ~/.local/share/bdl-pool-bore/0.6.0`.

Ce premier essai ne modifie ni le site, ni DuckDNS, ni l'API HTTPS. L'IP du domicile
reste visible tant que des enregistrements DNS pointent dessus. Masquer l'accès
Stratum seul ne masque pas l'API. Le relais public ne garantit ni réservation
de port ni disponibilité permanente. Bore ne chiffre pas lui-même le trafic ;
Stratum V2 conserve son chiffrement Noise.

Source : [documentation officielle de Bore](https://github.com/ekzhang/bore).
