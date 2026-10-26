# Vidéo 4ward : texte à apprendre (face cam)

Répartition : Clément 1-2 · Lancelot 3-4-5 · Hugo 6-7. Voix en anglais. La vidéo s'adapte à votre rythme, inutile de chronométrer au mot près : visez les durées indiquées, à environ 2,5 mots par seconde.

## 1. The problem · Clément · ~20 s

Meet Lucía. She imports tequila into Lisbon, and she owes her supplier in Guadalajara one million pesos, due in ninety days. If the peso rises ten percent before then, her invoice costs her five thousand five hundred dollars more. That's her whole margin, gone.

## 2. Why nobody helps her · Clément · ~25 s

A big company would call its bank and lock the exchange rate. Lucía can't. Banks want a credit line she doesn't have. FX fintechs want deposits and paperwork, and only cover a few currencies. Options cost a few percent of the invoice. So most small businesses do nothing, and just hope the rate doesn't move.

## 3. Why Stellar · Lancelot · ~22 s

Stellar already solves half of this: native USDC, path payments that deliver any currency in seconds, and almost no fees. Lucía can pay Guadalajara today, instantly. But only at today's rate. Nothing lets a small business lock today's rate for a payment it will make in ninety days.

## 4. The solution · Lancelot · ~22 s

That's 4ward. Lucía locks a fair ninety-day price. She deposits a small guarantee, which is also the most she can ever lose: no credit check, no margin calls. On the due date, the contract settles at that day's rate, and a path payment pays her supplier in pesos.

## 5. How it holds · Lancelot · ~22 s

It all runs on one Soroban contract. Prices come through Reflector's oracle interface, and the forward rate uses Etherfuse's real bond yields. Liquidity providers back every hedge and earn the spread, and the contract never promises more than their capital can cover.

## 6. Demo · Hugo · ~65 s (sur l'écran du site)

1. Here's Lucía. Her invoice: one million pesos in ninety days, locked at fifty-five thousand dollars, with ten percent protection.
2. One signature from her wallet, and it's locked.
3. Now the peso jumps twelve percent. That's the one thing we simulate: the peso rate, on a test oracle with the same interface as Reflector. Everything else is real.
4. At maturity, the contract reads that day's rate, and the pool pays Lucía the increase.
5. She pays her supplier from the same card: they receive exactly one million pesos.
6. Total cost: fifty-six thousand one hundred dollars, versus sixty-one thousand six hundred without protection.
7. On the other side, investors choose their risk, cautious or dynamic. And everything is verifiable on Stellar.

## 7. Closing · Hugo · ~6 s

4ward. Lock today's price for tomorrow's invoice.

---

## Pour l'enregistrement

- **Un fichier par section**, nommé `1-clement.mov`, `2-clement.mov`, `3-lancelot.mov`… Téléphone à l'horizontale, à hauteur des yeux, lumière de face, pièce calme. Micro du téléphone à 30-50 cm, ou des écouteurs filaires.
- **1 seconde de silence avant de parler et après la dernière phrase.** Une petite pause entre les phrases aide le montage.
- **Un raté ?** Ne coupez pas : marquez une pause, puis reprenez la phrase depuis le début. On retire le raté au montage.
- **Ne changez pas les chiffres ni les noms** (Lucía, Guadalajara, Reflector, Etherfuse, Soroban). Ils sont vérifiés. Le reste, vous pouvez le dire avec vos mots.
- Hugo (6-7) : la face cam est en vignette dans un coin pendant la démo. Le site reste au centre.
