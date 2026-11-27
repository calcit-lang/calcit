
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|WASI 0.2.12 HTTP proxy incoming-handler integration fixture") (:init-fn 'app.main/handle) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        '*hits $ %{} 'CodeEntry (:doc "|Persistent request counter across HTTP turns.")
          :code $ quote $ defref *hits 0
          :examples $ []
          :schema $ quote $ Ref Number
        'Payload $ %{} 'CodeEntry (:doc "|Decoded JSON/EDN request payload.")
          :code $ quote $ defstruct Payload (:name String) (:count Int32)
          :examples $ []
          :schema $ quote StructDef
        'Reply $ %{} 'CodeEntry (:doc "|Typed JSON/EDN response payload.")
          :code $ quote $ defstruct Reply (:ok Bool) (:greeting String) (:hits Int32)
          :examples $ []
          :schema $ quote StructDef
        'alloc-stress-chunk $ %{} 'CodeEntry (:doc "|64-byte ASCII chunk used for request-arena allocation stress.")
          :code $ quote $ def alloc-stress-chunk |0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
          :examples $ []
          :schema $ quote String
        'alloc-stress-loop $ %{} 'CodeEntry (:doc "|Allocate >256 KiB of ephemeral Strings in a request turn.")
          :code $ quote $ defn alloc-stress-loop (remaining total)
            if (<= remaining 0) total
              let
                  chunk $ str alloc-stress-chunk alloc-stress-chunk alloc-stress-chunk alloc-stress-chunk |: remaining
                recur (&- remaining 1) (&+ total (count chunk))
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'bump-hits! $ %{} 'CodeEntry (:doc "|Increment persistent request counter and return new count.")
          :code $ quote $ defn bump-hits! ()
            let
                next $ &+ @*hits 1
              reset! *hits next
              , next
          :examples $ []
          :schema $ quote $ Fn (-> Number) (:features :state)
        'extract-body-text $ %{} 'CodeEntry (:doc "|Extract UTF-8 text from HttpBody.")
          :code $ quote $ defn extract-body-text (body)
            match body
              (:text raw) (Result :ok raw)
              (:empty) (Result :err |empty-body)
              (:bytes _buf) (Result :err |binary-body)
          :examples $ []
          :schema $ quote $ Fn HttpBody (-> (Result String String))
        'find-header-at $ %{} 'CodeEntry (:doc "|Find first HttpHeader matching target name.")
          :code $ quote $ defn find-header-at (headers target idx)
            if (>= idx (count headers)) (Option :none)
              let
                  hdr $ &list:nth headers idx
                if (= (:name hdr) target)
                  Option :some (:value hdr)
                  recur headers target (&+ idx 1)
          :examples $ []
          :schema $ quote $ Fn (List HttpHeader) String Number (-> (Option String))
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
            HttpResponse :status status :headers ([] (HttpHeader :name |content-type :value "|text/plain; charset=utf-8")) :body (HttpBody :text text)
          :examples $ []
          :schema $ quote $ Fn Number String (-> HttpResponse)
        'handle $ %{} 'CodeEntry (:doc "|WASI HTTP proxy incoming-handler entry covering routing, headers, JSON/EDN, binary echo, errors, and request arena reset.")
          :code $ quote $ defn handle (req)
            let
                method $ method-name (:method req)
                path $ :path req
                headers $ :headers req
                body $ :body req
              cond
                (and (= method |GET) (= path |/health))
                  let
                      hits $ bump-hits!
                    Result :ok $ plain-response 200 (str |ok: hits)
                (and (= method |GET) (= path |/headers))
                  match (find-header-at headers |x-test 0)
                    (:some value)
                      Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value "|text/plain; charset=utf-8") (HttpHeader :name |x-echo :value value)) :body (HttpBody :text value)
                    (:none)
                      Result :ok $ plain-response 400 "|missing x-test"
                (and (= method |POST) (= path |/json))
                  match (extract-body-text body)
                    (:err reason) (Result :ok (plain-response 400 reason))
                    (:ok raw)
                      match (try-parse-json raw)
                        (:err reason) (Result :ok (plain-response 400 reason))
                        (:ok decoded)
                          match (try-decode-map-as decoded app.main/Payload)
                            (:err reason) (Result :ok (plain-response 400 reason))
                            (:ok payload)
                              match (number->int32 (bump-hits!))
                                (:err reason) (Result :ok (plain-response 500 reason))
                                (:ok hits)
                                  let
                                      reply $ Reply :ok true :greeting (str |hi- (:name payload) |# (:count payload)) :hits hits
                                    Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value |application/json)) :body (HttpBody :text (json-stringify (encode-map-as reply app.main/Reply)))
                (and (= method |POST) (= path |/edn))
                  match (extract-body-text body)
                    (:err reason) (Result :ok (plain-response 400 reason))
                    (:ok raw)
                      match (try-parse-cirru-edn-as raw app.main/Payload)
                        (:err reason) (Result :ok (plain-response 400 reason))
                        (:ok payload)
                          match (number->int32 (bump-hits!))
                            (:err reason) (Result :ok (plain-response 500 reason))
                            (:ok hits)
                              let
                                  reply $ Reply :ok true :greeting (str |hi- (:name payload) |# (:count payload)) :hits hits
                                Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value |application/edn)) :body (HttpBody :text (format-cirru-edn reply))
                (and (= method |POST) (= path |/echo-bytes))
                  match body
                    (:bytes buf)
                      Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value |application/octet-stream)) :body (HttpBody :bytes buf)
                    (:text text)
                      Result :ok $ plain-response 200 text
                    (:empty)
                      Result :ok $ HttpResponse :status 200 :headers ([]) :body (HttpBody :empty)
                (and (= method |GET) (= path |/alloc-stress))
                  let
                      total $ alloc-stress-loop 1024 0
                      hits $ bump-hits!
                    Result :ok $ plain-response 200 (str |stress: hits |: total)
                (and (= method |GET) (= path |/fail))
                  Result :err |boom-from-handler
                true
                  Result :ok $ plain-response 404 |not-found
          :examples $ []
          :schema $ quote $ Fn HttpRequest (-> (Result HttpResponse String)) (:features :state)
          :tests $ [] $ %{} 'TestEntry (:name |handles-routes-and-types-natively)
            :code $ quote $ do
              reset! *hits 0
              let
                  health-req $ HttpRequest :method (HttpMethod :get) :path |/health :headers ([]) :body (HttpBody :empty)
                  hdr-req $ HttpRequest :method (HttpMethod :get) :path |/headers :headers ([] (HttpHeader :name |x-test :value |alpha)) :body (HttpBody :empty)
                  json-req $ HttpRequest :method (HttpMethod :post) :path |/json :headers ([]) :body (HttpBody :text "|{\"name\":\"calcit\",\"count\":3}")
                  edn-req $ HttpRequest :method (HttpMethod :post) :path |/edn :headers ([]) :body (HttpBody :text "|%{} 'Payload (:name |edn-user) (:count 7)")
                  bin $ &buffer 0 255 128 65
                  bytes-req $ HttpRequest :method (HttpMethod :post) :path |/echo-bytes :headers ([]) :body (HttpBody :bytes bin)
                  fail-req $ HttpRequest :method (HttpMethod :get) :path |/fail :headers ([]) :body (HttpBody :empty)
                  miss-req $ HttpRequest :method (HttpMethod :other |CUSTOM) :path |/missing :headers ([]) :body (HttpBody :empty)
                assert=
                  Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value "|text/plain; charset=utf-8")) :body (HttpBody :text |ok:1)
                  handle health-req
                assert=
                  Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value "|text/plain; charset=utf-8") (HttpHeader :name |x-echo :value |alpha)) :body (HttpBody :text |alpha)
                  handle hdr-req
                match (handle json-req)
                  (:err msg) (raise msg)
                  (:ok resp)
                    do
                      assert= 200 $ :status resp
                      match (:body resp)
                        (:text json-text)
                          match (try-parse-json json-text)
                            (:err msg) (raise msg)
                            (:ok val)
                              match (try-decode-map-as val app.main/Reply)
                                (:err msg) (raise msg)
                                (:ok reply)
                                  assert= (Reply :ok true :greeting |hi-calcit#3 :hits (assert-type 2 Int32)) reply
                        (:empty) (raise |unexpected-empty)
                        (:bytes _b) (raise |unexpected-bytes)
                match (handle edn-req)
                  (:err msg) (raise msg)
                  (:ok resp)
                    do
                      assert= 200 $ :status resp
                      match (:body resp)
                        (:text edn-text)
                          assert= (Result :ok (Reply :ok true :greeting |hi-edn-user#7 :hits (assert-type 3 Int32))) (try-parse-cirru-edn-as edn-text app.main/Reply)
                        (:empty) (raise |unexpected-empty)
                        (:bytes _b) (raise |unexpected-bytes)
                assert=
                  Result :ok $ HttpResponse :status 200 :headers ([] (HttpHeader :name |content-type :value |application/octet-stream)) :body (HttpBody :bytes bin)
                  handle bytes-req
                assert= (Result :err |boom-from-handler) (handle fail-req)
                assert=
                  Result :ok $ HttpResponse :status 404 :headers ([] (HttpHeader :name |content-type :value "|text/plain; charset=utf-8")) :body (HttpBody :text |not-found)
                  handle miss-req
            :tags $ #{} :unit
        'reload! $ %{} 'CodeEntry (:doc "|No-op reload placeholder.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
