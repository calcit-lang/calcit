
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
                  calls $ atom 0
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
        'make-counter-host $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-counter-host (value) (&js-object)
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :file |js-ffi-assets/counter-host.js
          :schema $ :: 'Fn $ {} (:return 'JsObject)
            :args $ [] 'Number
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
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
