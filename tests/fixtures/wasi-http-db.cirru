
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {}
      :description "|WASI 0.2.12 HTTP+DB service incoming-handler integration fixture for calcit:db@0.1.0"
      :init-fn 'app.main/handle
      :mode :native
      :reload-fn 'app.main/reload!
      :target :wasm
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        '*hits $ %{} 'CodeEntry
          :doc "|Persistent request counter across HTTP+DB turns."
          :code $ quote $ defref *hits 0
          :examples $ []
          :schema $ quote $ Ref Number
        'CreateItemPayload $ %{} 'CodeEntry (:doc "|Decoded JSON request body for creating an item.")
          :code $ quote $ defstruct CreateItemPayload (:name String)
          :examples $ []
          :schema $ quote StructDef
        'DbError $ %{} 'CodeEntry
          :doc "|Structured error returned by calcit:db@0.1.0 host calls and row decoding."
          :code $ quote $ defenum DbError (:backend String) (:capability-denied String) (:constraint-violation String) (:decode-error String) (:invalid-query String) (:timeout String) (:tx-error String) (:unsupported String)
          :examples $ []
          :schema $ quote EnumDef
        'DbExecResult $ %{} 'CodeEntry
          :doc "|Structured mutation metadata returned by sql-exec and tx-exec."
          :code $ quote $ defstruct DbExecResult
            :last-insert-id $ Option Int64
            :rows-affected UInt64
          :examples $ []
          :schema $ quote StructDef
        'DbParam $ %{} 'CodeEntry
          :doc "|Parameterized SQL bind parameter for calcit:db/sql@0.1.0."
          :code $ quote $ defenum DbParam (:bool-value Bool) (:bytes-value Buffer) (:float-value Float64) (:int-value Int64) (:null-value) (:text-value String)
          :examples $ []
          :schema $ quote EnumDef
        'DbQueryResult $ %{} 'CodeEntry
          :doc "|Structured query result payload containing ordered column names and JSON rows."
          :code $ quote $ defstruct DbQueryResult
            :columns $ List String
            :rows-json String
          :examples $ []
          :schema $ quote StructDef
        'InsertReply $ %{} 'CodeEntry (:doc "|Typed JSON response for POST /items.")
          :code $ quote $ defstruct InsertReply
            :last-insert-id $ Option Int64
            :rows-affected UInt64
          :examples $ []
          :schema $ quote StructDef
        'ItemRow $ %{} 'CodeEntry (:doc "|Typed SQLite item row decoded via query-as.")
          :code $ quote $ defstruct ItemRow (:id Int64) (:name String)
          :examples $ []
          :schema $ quote StructDef
        'ItemsReply $ %{} 'CodeEntry
          :doc "|Typed JSON response for GET /items and POST /tx/commit."
          :code $ quote $ defstruct ItemsReply (:hits Int32)
            :items $ List app.main/ItemRow
          :examples $ []
          :schema $ quote StructDef
        'bump-hits! $ %{} 'CodeEntry
          :doc "|Increment persistent request counter and return new count."
          :code $ quote $ defn bump-hits! ()
            let
                next $ &+ @*hits 1
              reset! *hits next
              , next
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'db-error-response $ %{} 'CodeEntry
          :doc "|Map a DbError variant to a deterministic HTTP error response."
          :code $ quote $ defn db-error-response (err)
            match err
              (:capability-denied msg) (plain-response 403 msg)
              (:constraint-violation msg) (plain-response 409 msg)
              (:invalid-query msg) (plain-response 400 msg)
              (:decode-error msg) (plain-response 502 msg)
              (:timeout msg) (plain-response 504 msg)
              (:tx-error msg) (plain-response 500 msg)
              (:backend msg) (plain-response 500 msg)
              (:unsupported msg) (plain-response 500 msg)
          :examples $ []
          :schema $ quote $ Fn DbError (-> HttpResponse)
          :tests $ [] $ %{} 'TestEntry (:name |maps-all-db-error-variants-to-status-codes)
            :code $ quote $ do
              assert= (plain-response 403 |denied)
                db-error-response $ DbError :capability-denied |denied
              assert= (plain-response 409 |unique)
                db-error-response $ DbError :constraint-violation |unique
              assert= (plain-response 400 |syntax)
                db-error-response $ DbError :invalid-query |syntax
              assert= (plain-response 502 |json)
                db-error-response $ DbError :decode-error |json
              assert= (plain-response 504 |slow)
                db-error-response $ DbError :timeout |slow
              assert= (plain-response 500 |tx)
                db-error-response $ DbError :tx-error |tx
              assert= (plain-response 500 |io)
                db-error-response $ DbError :backend |io
              assert= (plain-response 500 |unsup)
                db-error-response $ DbError :unsupported |unsup
            :tags $ #{} :unit
        'extract-body-bytes $ %{} 'CodeEntry (:doc "|Extract binary Buffer from HttpBody.")
          :code $ quote $ defn extract-body-bytes (body)
            match body
              (:bytes buf) (Result :ok buf)
              (:text _raw) (Result :err |text-body)
              (:empty) (Result :err |empty-body)
          :examples $ []
          :schema $ quote $ Fn HttpBody
            -> $ Result Buffer String
          :tests $ [] $ %{} 'TestEntry (:name |extracts-bytes-and-rejects-text-or-empty)
            :code $ quote $ do
              assert=
                Result :ok $ &buffer 1 2
                extract-body-bytes $ HttpBody :bytes $ &buffer 1 2
              assert= (Result :err |text-body)
                extract-body-bytes $ HttpBody :text |ignored
              assert= (Result :err |empty-body)
                extract-body-bytes $ HttpBody :empty
            :tags $ #{} :unit
        'extract-body-text $ %{} 'CodeEntry (:doc "|Extract UTF-8 text from HttpBody.")
          :code $ quote $ defn extract-body-text (body)
            match body
              (:text raw) (Result :ok raw)
              (:empty) (Result :err |empty-body)
              (:bytes _buf) (Result :err |binary-body)
          :examples $ []
          :schema $ quote $ Fn HttpBody
            -> $ Result String String
        'handle $ %{} 'CodeEntry
          :doc "|WASI HTTP+DB incoming-handler exercising SQLite, transactions, KV, default-deny errors, and arena reuse."
          :code $ quote $ defn handle (req)
            let
                method $ method-name $ :method req
                path $ :path req
                body $ :body req
              cond
                  and (= method |GET) (= path |/health)
                  let
                      hits $ bump-hits!
                    Result :ok $ plain-response 200 $ str |ok: hits
                (and (= method |POST) (= path |/items))
                  match (extract-body-text body)
                    (:err reason)
                      Result :ok $ plain-response 400 reason
                    (:ok raw)
                      match (try-parse-json raw)
                        (:err reason)
                          Result :ok $ plain-response 400 reason
                        (:ok decoded)
                          match (try-decode-map-as decoded app.main/CreateItemPayload)
                            (:err reason)
                              Result :ok $ plain-response 400 reason
                            (:ok payload)
                              match
                                sql-exec "|INSERT INTO items (name) VALUES (?)" $ [] $ DbParam :text-value (:name payload)
                                (:err e)
                                  Result :ok $ db-error-response e
                                (:ok exec-res)
                                  let
                                      reply $ InsertReply :last-insert-id (:last-insert-id exec-res) :rows-affected $ :rows-affected exec-res
                                    Result :ok $ json-response 201 $ json-stringify (encode-map-as reply app.main/InsertReply)
                (and (= method |GET) (= path |/items))
                  match
                    query-as "|SELECT id, name FROM items ORDER BY id" ([]) app.main/ItemRow
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok rows)
                      match
                        number->int32 $ bump-hits!
                        (:err reason)
                          Result :ok $ plain-response 500 reason
                        (:ok hits)
                          let
                              reply $ ItemsReply :hits hits :items rows
                            Result :ok $ json-response 200 $ json-stringify (encode-map-as reply app.main/ItemsReply)
                (and (= method |POST) (= path |/ddl))
                  match
                    sql-exec "|CREATE TABLE forbidden (id INTEGER)" $ []
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok _res)
                      Result :ok $ plain-response 200 |unexpected-ddl
                (and (= method |POST) (= path |/tx/commit))
                  match (tx-begin)
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok tx-id)
                      match
                        tx-exec tx-id "|INSERT INTO items (name) VALUES (?)" $ [] $ DbParam :text-value |tx-committed
                        (:err e)
                          let
                              _rb $ tx-rollback tx-id
                            Result :ok $ db-error-response e
                        (:ok _res)
                          match
                            tx-query-as tx-id "|SELECT id, name FROM items ORDER BY id" ([]) app.main/ItemRow
                            (:err e)
                              let
                                  _rb $ tx-rollback tx-id
                                Result :ok $ db-error-response e
                            (:ok rows)
                              match (tx-commit tx-id)
                                (:err e)
                                  Result :ok $ db-error-response e
                                (:ok _u)
                                  match
                                    number->int32 $ bump-hits!
                                    (:err reason)
                                      Result :ok $ plain-response 500 reason
                                    (:ok hits)
                                      let
                                          reply $ ItemsReply :hits hits :items rows
                                        Result :ok $ json-response 200 $ json-stringify (encode-map-as reply app.main/ItemsReply)
                (and (= method |POST) (= path |/tx/rollback))
                  match (tx-begin)
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok tx-id)
                      match
                        tx-exec tx-id "|INSERT INTO items (name) VALUES (?)" $ [] $ DbParam :text-value |tx-rolled-back
                        (:err e)
                          let
                              _rb $ tx-rollback tx-id
                            Result :ok $ db-error-response e
                        (:ok _res)
                          match (tx-rollback tx-id)
                            (:err e)
                              Result :ok $ db-error-response e
                            (:ok _u)
                              Result :ok $ plain-response 200 |rolled-back
                (and (= method |POST) (= path |/tx/leak))
                  match (tx-begin)
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok tx-id)
                      match
                        tx-exec tx-id "|INSERT INTO items (name) VALUES (?)" $ [] $ DbParam :text-value |tx-leaked
                        (:err e)
                          Result :ok $ db-error-response e
                        (:ok _res)
                          Result :ok $ plain-response 200 |leaked
                (and (= method |POST) (= path |/kv))
                  match (extract-body-bytes body)
                    (:err reason)
                      Result :ok $ plain-response 400 reason
                    (:ok buf)
                      match (kv-set |app:session buf None)
                        (:err e)
                          Result :ok $ db-error-response e
                        (:ok _u)
                          Result :ok $ plain-response 200 |stored
                (and (= method |GET) (= path |/kv))
                  match (kv-get |app:session)
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok opt-buf)
                      match opt-buf
                        (:none)
                          Result :ok $ plain-response 404 |missing
                        (:some buf)
                          Result :ok $ HttpResponse :status 200 :headers
                            [] $ HttpHeader :name |content-type :value |application/octet-stream
                            , :body $ HttpBody :bytes buf
                (and (= method |DELETE) (= path |/kv))
                  match (kv-delete |app:session)
                    (:err e)
                      Result :ok $ db-error-response e
                    (:ok deleted?)
                      Result :ok $ plain-response 200 $ if deleted? |true |false
                (and (= method |POST) (= path |/kv/denied))
                  match (extract-body-bytes body)
                    (:ok buf)
                      match (kv-set |secret:token buf None)
                        (:err e)
                          Result :ok $ db-error-response e
                        (:ok _u)
                          Result :ok $ plain-response 200 |unexpected-allow
                    (:err _reason)
                      match (kv-get |secret:token)
                        (:err e)
                          Result :ok $ db-error-response e
                        (:ok _opt)
                          Result :ok $ plain-response 200 |unexpected-allow
                true $ Result :ok $ plain-response 404 |not-found
          :examples $ []
          :schema $ quote $ Fn HttpRequest
            -> $ Result HttpResponse String
          :tests $ [] $ %{} 'TestEntry (:name |handles-pure-health-and-fallback-natively)
            :code $ quote $ do (reset! *hits 0)
              assert=
                Result :ok $ plain-response 200 |ok:1
                handle $ HttpRequest :method (HttpMethod :get) :path |/health :headers ([]) :body $ HttpBody :empty
              assert=
                Result :ok $ plain-response 404 |not-found
                handle $ HttpRequest :method (HttpMethod :get) :path |/unknown :headers ([]) :body $ HttpBody :empty
            :tags $ #{} :unit
        'json-response $ %{} 'CodeEntry (:doc "|Construct a JSON HttpResponse.")
          :code $ quote $ defn json-response (status text)
            HttpResponse :status status :headers
              [] $ HttpHeader :name |content-type :value |application/json
              , :body $ HttpBody :text text
          :examples $ []
          :schema $ quote $ Fn Number String (-> HttpResponse)
        'kv-delete $ %{} 'CodeEntry (:doc "|Delete a key under calcit:db/kv@0.1.0.")
          :code $ quote $ defwasm-import kv-delete (key) |calcit:db/kv@0.1.0 |delete
          :examples $ []
          :schema $ quote $ Fn String
            -> $ Result Bool DbError
        'kv-get $ %{} 'CodeEntry (:doc "|Read a key under calcit:db/kv@0.1.0.")
          :code $ quote $ defwasm-import kv-get (key) |calcit:db/kv@0.1.0 |get
          :examples $ []
          :schema $ quote $ Fn String
            -> $ Result (Option Buffer) DbError
        'kv-set $ %{} 'CodeEntry
          :doc "|Write a key with optional millisecond TTL under calcit:db/kv@0.1.0."
          :code $ quote $ defwasm-import kv-set (key value ttl-ms) |calcit:db/kv@0.1.0 |set
          :examples $ []
          :schema $ quote $ Fn String Buffer (Option UInt64)
            -> $ Result Unit DbError
        'method-name $ %{} 'CodeEntry (:doc "|Format HttpMethod as uppercase ASCII string.")
          :code $ quote $ defn method-name (method)
            match method
              (:get) |GET
              (:head) |HEAD
              (:post) |POST
              (:put) |PUT
              (:delete) |DELETE
              (:connect) |CONNECT
              (:options) |OPTIONS
              (:trace) |TRACE
              (:patch) |PATCH
              (:other name) name
          :examples $ []
          :schema $ quote $ Fn HttpMethod (-> String)
        'plain-response $ %{} 'CodeEntry (:doc "|Construct a plain-text HttpResponse.")
          :code $ quote $ defn plain-response (status text)
            HttpResponse :status status :headers
              [] $ HttpHeader :name |content-type :value "|text/plain; charset=utf-8"
              , :body $ HttpBody :text text
          :examples $ []
          :schema $ quote $ Fn Number String (-> HttpResponse)
        'query-as $ %{} 'CodeEntry
          :doc "|Execute sql-query and decode JSON rows into Result (List RowStruct) DbError."
          :code $ quote $ defmacro query-as (sql params row-type)
            quasiquote $ match (app.main/sql-query ~sql ~params)
              (:err db-err) (Result :err db-err)
              (:ok res)
                match
                  try-parse-json $ :rows-json res
                  (:err reason)
                    Result :err $ app.main/DbError :decode-error reason
                  (:ok parsed)
                    match
                      try-decode-map-as parsed $ List ~row-type
                      (:err reason)
                        Result :err $ app.main/DbError :decode-error reason
                      (:ok rows) (Result :ok rows)
          :examples $ []
          :schema $ quote $ Macro (Expr String)
            Expr $ List DbParam
            , Syntax
              -> $ Expr $ Result Dynamic DbError
        'reload! $ %{} 'CodeEntry (:doc "|No-op reload placeholder.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'sql-exec $ %{} 'CodeEntry
          :doc "|Execute a parameterized write or DDL SQL statement via calcit:db/sql@0.1.0."
          :code $ quote $ defwasm-import sql-exec (sql params) |calcit:db/sql@0.1.0 |exec
          :examples $ []
          :schema $ quote $ Fn String (List DbParam)
            -> $ Result DbExecResult DbError
        'sql-query $ %{} 'CodeEntry
          :doc "|Execute a parameterized SQL query via calcit:db/sql@0.1.0."
          :code $ quote $ defwasm-import sql-query (sql params) |calcit:db/sql@0.1.0 |query
          :examples $ []
          :schema $ quote $ Fn String (List DbParam)
            -> $ Result DbQueryResult DbError
        'tx-begin $ %{} 'CodeEntry
          :doc "|Open a host-managed SQLite savepoint transaction via calcit:db/sql@0.1.0."
          :code $ quote $ defwasm-import tx-begin () |calcit:db/sql@0.1.0 |tx-begin
          :examples $ []
          :schema $ quote $ Fn
            -> $ Result UInt64 DbError
        'tx-commit $ %{} 'CodeEntry
          :doc "|Commit an active host-managed transaction via calcit:db/sql@0.1.0."
          :code $ quote $ defwasm-import tx-commit (tx-id) |calcit:db/sql@0.1.0 |tx-commit
          :examples $ []
          :schema $ quote $ Fn UInt64
            -> $ Result Unit DbError
        'tx-exec $ %{} 'CodeEntry
          :doc "|Execute a parameterized SQL write or DDL statement inside an active transaction."
          :code $ quote $ defwasm-import tx-exec (tx-id sql params) |calcit:db/sql@0.1.0 |tx-exec
          :examples $ []
          :schema $ quote $ Fn UInt64 String (List DbParam)
            -> $ Result DbExecResult DbError
        'tx-query $ %{} 'CodeEntry
          :doc "|Execute a parameterized SQL query inside an active transaction."
          :code $ quote $ defwasm-import tx-query (tx-id sql params) |calcit:db/sql@0.1.0 |tx-query
          :examples $ []
          :schema $ quote $ Fn UInt64 String (List DbParam)
            -> $ Result DbQueryResult DbError
        'tx-query-as $ %{} 'CodeEntry
          :doc "|Execute tx-query and decode JSON rows into Result (List RowStruct) DbError."
          :code $ quote $ defmacro tx-query-as (tx-id sql params row-type)
            quasiquote $ match (app.main/tx-query ~tx-id ~sql ~params)
              (:err db-err) (Result :err db-err)
              (:ok res)
                match
                  try-parse-json $ :rows-json res
                  (:err reason)
                    Result :err $ app.main/DbError :decode-error reason
                  (:ok parsed)
                    match
                      try-decode-map-as parsed $ List ~row-type
                      (:err reason)
                        Result :err $ app.main/DbError :decode-error reason
                      (:ok rows) (Result :ok rows)
          :examples $ []
          :schema $ quote $ Macro (Expr UInt64) (Expr String)
            Expr $ List DbParam
            , Syntax
              -> $ Expr $ Result Dynamic DbError
        'tx-rollback $ %{} 'CodeEntry
          :doc "|Roll back an active host-managed transaction via calcit:db/sql@0.1.0."
          :code $ quote $ defwasm-import tx-rollback (tx-id) |calcit:db/sql@0.1.0 |tx-rollback
          :examples $ []
          :schema $ quote $ Fn UInt64
            -> $ Result Unit DbError
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
