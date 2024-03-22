# learn_zero2prod

Me going through the book Zero to Production in Rust by Luca Palmieri.

[Book's page](https://www.zero2prod.com/index.html?country=Slovakia%20&discount_code=EEU60) \
[Author's book repo](https://github.com/LukeMathWalker/zero-to-production)

## Questions from the end of chapter 7
- What happens if a user tries to subscribe twice? Make sure that they receive two confirmation emails
    - Client will receive HTTP Status 500 because there is a unique index on email
- What happens if a user clicks on a confirmation link twice?
- What happens if the subscription token is well-formatted but non-existent?
- Add validation on the incoming token, we are currently passing the raw user input straight into a query (sqlx is protecting us from SQL injection)
- User a proper templating solution for our emails (e.g. [tera](https://github.com/Keats/tera))