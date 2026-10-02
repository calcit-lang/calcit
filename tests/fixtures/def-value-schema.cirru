
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'answer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def answer 42
          :examples $ []
          :schema $ :: 'Number
        'initial-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def initial-state values/initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
        'label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def label |Ada
          :examples $ []
          :schema $ :: 'String
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (verify-values) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'members $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def members
            {} $ 1 |Ada
          :examples $ []
          :schema $ :: 'Map 'Number 'String
        'open-store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def open-store members
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'state-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
        'verify-login-decode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-login-decode ()
            match
              try-decode-map-as
                {} (:username |Ada) (:password |secret)
                , app.values/LoginState
              (:ok state)
                assert= |Ada $ :username state
              (:err reason) (raise reason)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |checked-login-projection)
              :code $ quote $ do (verify-login-decode)
                match
                  try-decode-map-as
                    {} (:username |Ada) (:password 42)
                    , app.values/LoginState
                  (:ok _) (raise |invalid-password-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |password
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |persisted-nested-value)
              :code $ quote $ do
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ {} $ :years ([] 2025 2026)
                    :: 'Map 'Tag $ :: 'List 'Number
                  (:ok data)
                    assert=
                      Option :some $ [] 2025 2026
                      get data :years
                  (:err reason) (raise reason)
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ {} $ :years ([] 2025 |bad)
                    :: 'Map 'Tag $ :: 'List 'Number
                  (:ok _) (raise |invalid-storage-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |[1]
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |member-key-contract)
              :code $ quote $ do
                match
                  try-decode-map-as members $ :: 'Map 'Number 'String
                  (:ok data)
                    assert= (Option :some |Ada) (get data 1)
                  (:err reason) (raise reason)
                match
                  try-decode-map-as members $ :: 'Map 'String 'String
                  (:ok _) (raise |numeric-member-key-accepted-as-string)
                  (:err reason)
                    assert= true $ .includes? reason |key
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |restored-credentials)
              :code $ quote $ do
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ [] |Ada |secret
                    :: 'List 'String
                  (:ok credentials)
                    assert= ([] |Ada |secret) credentials
                  (:err reason) (raise reason)
                match
                  try-parse-cirru-edn-as
                    format-cirru-edn $ [] |Ada 42
                    :: 'List 'String
                  (:ok _) (raise |invalid-credentials-accepted)
                  (:err reason)
                    assert= true $ .includes? reason |[1]
              :tags $ #{} :diary-boundary :unit
            %{} 'TestEntry (:name |nominal-component-state)
              :code $ quote $ do (verify-values)
                assert= |Ada $ :username $ assoc initial-state :username |Ada
                assert= | $ :username initial-state
              :tags $ #{} :diary-boundary :unit
        'verify-values $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn verify-values ()
            assert= | $ :username initial-state
            assert= | $ :username state-alias
            assert= | $ :password reader/state-alias
            assert= 1 $ .len members
            assert= 42 answer
            assert= |Ada label
            assert= members open-store
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |retains-typed-top-level-values)
            :code $ quote $ verify-values
            :tags $ #{} :def-value-contract :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require (app.reader :as reader) (app.values :as values)
    'app.reader $ %{} 'FileEntry
      :defs $ {} $ 'state-alias
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def state-alias values/initial-state
          :examples $ []
          :schema $ :: 'app.values/LoginState
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.reader
          :require $ app.values :as values
    'app.values $ %{} 'FileEntry
      :defs $ {}
        'LoginState $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LoginState (:username 'String) (:password 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'initial-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def initial-state (LoginState :username | :password |)
          :examples $ []
          :schema $ :: 'app.values/LoginState
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.values
