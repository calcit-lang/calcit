

{} (:package |test-json)
  :configs $ {} (:init-fn |test-json.main/main!) (:reload-fn |test-json.main/reload!)
  :files $ {}
    |test-json.main $ {}
      :ns $ quote
        ns test-json.main $ :require
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-json $ quote
          fn ()
            assert=
              parse-json "|{\"a\": [1, 2], \":b\": 3}"
              {}
                |a $ [] 1 2
                :b 3
            &let
              data $ {}
                |a 1
                :b 2
                :c :k
              assert= data $ parse-json $ stringify-json data true
            &let
              data $ {}
                |a 1
                :b 2
                :c :k

              assert=
                parse-json $ stringify-json data
                {}
                  |a 1
                  |b 2
                  |c |k

        |main! $ quote
          defn main! ()
            log-title "|Testing json"
            test-json

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
