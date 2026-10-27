
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :node)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.api $ %{} 'FileEntry
      :defs $ {}
        'file-label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn file-label (x) (base-name x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'next-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn next-count () (count-a)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'plus-four $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plus-four (x)
            plus-two $ plus-two x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.api
          :require $ app.main :refer $ plus-two base-name count-a
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'CounterHost $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait CounterHost (:total 'Number)
            .add! $ :: 'Fn $ {}
              :args $ [] 'CounterHost 'Number
              :return 'Number
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :node)
            :names $ {} $ :total |total-value
          :schema $ :: 'Trait
        'RestHost $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait RestHost
            .emit $ :: 'Fn $ {}
              :args $ [] 'RestHost
              :rest 'String
              :return 'String
            .prefix $ :: 'Fn $ {}
              :args $ [] 'RestHost 'String
              :rest 'String
              :return 'String
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :node)
          :schema $ :: 'Trait
        'TallyHost $ %{} 'CodeEntry
          :doc "|External-object trait that inherits CounterHost members"
          :code $ quote $ deftrait TallyHost (requires CounterHost)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :node)
          :schema $ :: 'Trait
        'base-name $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn base-name (x) x
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} (:file |js-ffi-assets/base-name.js)
              :modules $ {} $ :path |node:path
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
            :features $ #{} :js-ffi
        'checked-counter-host $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-counter-host (value) (js-cast value 'CounterHost)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.main/CounterHost)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
          :tests $ []
            %{} 'TestEntry (:name |mapped-field)
              :code $ quote $ let
                  host $ checked-counter-host $ make-counter-host 3
                assert= 3 $ host :total
              :tags $ #{} :unit
            %{} 'TestEntry (:name |method-receiver)
              :code $ quote $ let
                  host $ checked-counter-host $ make-counter-host 3
                assert= 7 $ host .add! 4
              :tags $ #{} :unit
            %{} 'TestEntry (:name |single-evaluation)
              :code $ quote $ let
                  calls $ ref 0
                  host $ js-cast
                    do (swap! calls inc) (make-counter-host 3)
                    , 'CounterHost
                assert= 1 $ deref calls
                assert= 3 $ host :total
              :tags $ #{} :unit
            %{} 'TestEntry (:name |explicit-identity)
              :code $ quote $ let
                  host $ checked-counter-host $ make-counter-host 3
                  other $ checked-counter-host $ make-counter-host 3
                assert= true $ identical? host host
                assert= false $ identical? host other
              :tags $ #{} :unit
            %{} 'TestEntry (:name |inherited-members)
              :code $ quote $ let
                  tally $ js-cast (make-counter-host 3) 'TallyHost
                assert= 3 $ counter-total tally
                assert= 3 $ tally :total
                assert= 7 $ tally .add! 4
              :tags $ #{} :unit
            %{} 'TestEntry (:name |callable-host)
              :code $ quote $ let
                  host $ checked-counter-host $ make-callable-counter-host 3
                assert= 3 $ host :total
                assert= 7 $ host .add! 4
              :tags $ #{} :unit
        'checked-rest-host $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-rest-host (value) (js-cast value 'RestHost)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.main/RestHost)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
        'count-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-a () 0
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/count.js
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
            :features $ #{} :js-ffi
        'count-b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-b () 0
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/count.js
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
            :features $ #{} :js-ffi
        'counter-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn counter-add (host amount) (host .add! amount)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'app.main/CounterHost 'Number
            :features $ #{} :js-ffi
          :tests $ []
            %{} 'TestEntry (:name |declared-method-result-and-once-only-argument)
              :code $ quote $ let
                  calls $ ref 0
                  host $ checked-counter-host $ make-counter-host 3
                assert= 7 $ counter-add host $ do (swap! calls inc) 4
                assert= 1 $ deref calls
                assert= 3 $ counter-total host
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |inherited-receiver-and-prefix-result)
              :code $ quote $ let
                  tally $ js-cast (make-counter-host 3) 'TallyHost
                assert= 8 $ counter-add tally 5
                assert= 5 $ counter-add-prefix tally 2
                assert= 3 $ counter-total tally
              :tags $ #{} :js-ffi :unit
        'counter-add-prefix $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn counter-add-prefix (host amount) (.add! host amount)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'app.main/CounterHost 'Number
            :features $ #{} :js-ffi
        'counter-total $ %{} 'CodeEntry
          :doc "|Accepts any CounterHost, including traits that require it"
          :code $ quote $ defn counter-total (host) (host :total)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'app.main/CounterHost
            :features $ #{} :js-ffi
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            assert= 3 $ plus-one 2
            assert= 4 $ plus-two 2
            println |js-ffi-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'make-callable-counter-host $ %{} 'CodeEntry
          :doc "|Callable host export carrying CounterHost members, like an SDK default export"
          :code $ quote $ defn make-callable-counter-host (value) (&js-object)
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/callable-counter-host.js
          :schema $ :: 'Fn $ {} (:return 'JsObject)
            :args $ [] 'Number
            :features $ #{} :js-ffi
        'make-counter-host $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-counter-host (value) (&js-object)
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/counter-host.js
          :schema $ :: 'Fn $ {} (:return 'JsObject)
            :args $ [] 'Number
            :features $ #{} :js-ffi
        'make-rest-host $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-rest-host () (&js-object)
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/rest-host.js
          :schema $ :: 'Fn $ {} (:return 'JsObject)
            :args $ []
            :features $ #{} :js-ffi
        'plus-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plus-one (x) x
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :inline "|(x)=>x+1"
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
            :features $ #{} :js-ffi
        'plus-two $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plus-two (x) x
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/add-two.js
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
            :features $ #{} :js-ffi
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reload)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'rest-host-output $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rest-host-output (host items) (.emit host & items)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'app.main/RestHost $ :: 'List 'String
            :features $ #{} :js-ffi
          :tests $ []
            %{} 'TestEntry (:name |no-rest)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                assert= | $ .emit host
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |direct-rest)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                assert= |one:two $ .emit host |one |two
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |list-rest)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                assert= |one:two $ rest-host-output host $ [] |one |two
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |empty-list)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                assert= | $ rest-host-output host $ []
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |explicit-before-list)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                  items $ [] |two
                assert= |one:two $ .emit host |one & items
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |required-prefix)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                  items $ [] |one |two
                assert= |prefix:one:two $ .prefix host |prefix & items
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |prefix-no-rest)
              :code $ quote $ let
                  host $ checked-rest-host $ make-rest-host
                assert= |prefix $ .prefix host |prefix
              :tags $ #{} :js-ffi :unit
            %{} 'TestEntry (:name |single-evaluation)
              :code $ quote $ let
                  calls $ ref 0
                  host $ checked-rest-host $ make-rest-host
                  supply $ fn ()
                    hint-fn $ quote $ Fn
                      -> $ List String
                    swap! calls inc
                    [] |one
                assert= |one $ .emit host & $ supply
                assert= 1 $ deref calls
              :tags $ #{} :js-ffi :unit
        'unit-adapter $ %{} 'CodeEntry
          :doc "|The JS implementation returns Unit; the incompatible shadow body is intentionally never selected."
          :code $ quote $ defn unit-adapter () 42
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :inline "|() => undefined"
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |selected-js-returns-unit)
            :code $ quote $ assert= &unit (unit-adapter)
            :tags $ #{} :js-ffi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
