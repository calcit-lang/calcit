
{} (:package |test-string)
  :configs $ {} (:init-fn |test-string.main/main!) (:reload-fn |test-string.main/reload!)
  :files $ {}
    |test-string.main $ {}
      :ns $ quote
        ns test-string.main $ :require
          [] util.core :refer $ [] inside-eval:
      :defs $ {}

        |test-str $ quote
          defn test-str ()
            assert= (&str:concat |a |b) |ab
            assert= (&str:concat 1 2) |12
            assert= (str |a |b |c) |abc
            assert= (str |a nil |c) |ac
            assert= (str 1 2 3) |123
            assert= (type-of (&str 1)) :string
            assert=
              .replace "|this is a" |is |IS
              , "|thIS IS a"
            assert=
              split "|a,b,c" "|,"
              [] |a |b |c
            assert=
              split-lines "|a\nb\nc"
              [] |a |b |c
            assert=
              split |a中b文c |
              [] |a |中 |b |文 |c
            assert= 4
              count |good
            assert= |56789 $ .slice |0123456789 5
            assert= |567 $ .slice |0123456789 5 8
            assert= | $ .slice |0123456789 10
            assert= | $ .slice |0123456789 9 1
            assert= -1 $ &str:compare |a |b
            assert= 1 $ &str:compare |b |a
            assert= 0 $ &str:compare |a |a

            assert-detect identity $ < |a |b
            assert-detect identity $ < |aa |ab
            assert-detect not $ > |aa |ab

        |test-includes $ quote
          fn ()
            log-title "|Testing includes"

            assert= true $ includes? |abc |abc
            assert= false $ includes? |abd |abc

            assert= 3 $ .find-index |0123456 |3
            assert= 3 $ .find-index |0123456 |34
            assert= 0 $ .find-index |0123456 |01
            assert= 4 $ .find-index |0123456 |456
            assert= -1 $ .find-index |0123456 |98

            assert= true $ starts-with? |01234 |0
            assert= true $ starts-with? |01234 |01
            assert= false $ starts-with? |01234 |12

            assert= true $ ends-with? |01234 |34
            assert= true $ ends-with? |01234 |4
            assert= false $ ends-with? |01234 |23

            assert= |abc $ strip-prefix |ababc |ab
            assert= |0abc $ strip-prefix |0abc |ab

            assert= |aba $ strip-suffix |ababc |bc
            assert= |abc0 $ strip-suffix |abc0 |bc

        |test-parse $ quote
          fn ()
            assert= 0 $ parse-float |0

        |test-trim $ quote
          fn ()
            assert= | $ trim "|    "
            assert= |1 $ trim "|  1  "
            assert= |1 $ trim "|\n1\n"

            assert= | $ trim "|______" |_
            assert= |1 $ trim "|__1__" |_

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-format $ quote
          fn ()
            log-title "|Testing format"

            assert= |1.2346 $ .format 1.23456789 4
            assert= |1.235 $ .format 1.23456789 3
            assert= |1.23 $ .format 1.23456789 2
            assert= |1.2 $ .format 1.23456789 1

            inside-eval:

              ; TODO not stable
              ; assert= "|({} (:c ([] 3)) (:a 1) (:b |2) (:d ({} (([] 1 2) 3))))"
                pr-str $ {}
                  :a 1
                  :b |2
                  :c $ [] 3
                  :d $ {}
                    ([] 1 2) 3

            let
                Person $ new-record 'Person :name :age
                edn-demo "|%{} :Person (:age 23) (:name |Chen)"

              ; "no stable order"
              assert=
                count $ pr-str $ %{} Person (:name |Chen) (:age 23)
                count "|(%{} :Person (:name |Chen) (:age 23))"
              ; "no stable order"
              assert=
                count edn-demo
                count $ trim $ format-cirru-edn $ %{} Person (:name |Chen) (:age 23)

              assert=
                parse-cirru-edn edn-demo
                %{} Person (:name |Chen) (:age 23)

              assert= 'a
                parse-cirru-edn "|do 'a"

              assert=
                {}
                  :code $ :: 'quote
                    [] |+ |1 |2 |3
                parse-cirru-edn "|{} $ :code $ quote $ + 1 2 3"

              assert=
                :: :a 1
                parse-cirru-edn "|:: :a 1"

              assert= "|{} $ :code\n  quote $ + 1 2 3"
                trim $ format-cirru-edn $ {}
                  :code $ :: 'quote $ [] |+ |1 |2 |3

              assert= "|[] 'a"
                trim $ format-cirru-edn $ [] 'a

              assert= "|do nil"
                trim $ format-cirru-edn nil

              assert= "|do 's"
                trim $ format-cirru-edn 's

              assert= "|:: :&core-list-class $ [] 1 2 3"
                trim $ format-cirru-edn $ :: &core-list-class $ [] 1 2 3

              assert= (.escape "|\n") "|\"\\n\""
              assert= (.escape "|\t") "|\"\\t\""
              assert= (.escape "|a") "|\"a\""

        |test-char $ quote
          fn ()
            log-title "|Test char"

            assert= 97 $ .get-char-code |a
            assert= 27721 $ .get-char-code |汉

            assert= |a $ char-from-code 97

            assert= |a $ nth |abc 0
            assert= |b $ nth |abc 1
            assert= |a $ first |abc
            assert= |c $ last |abc
            assert= nil $ first |
            assert= nil $ last |

        |test-whitespace $ quote
          fn ()
            log-title "|Test blank?"

            assert-detect identity $ blank? |
            assert-detect identity $ blank? "\""
            assert-detect identity $ blank? "| "
            assert-detect identity $ blank? "|  "
            assert-detect identity $ blank? "|\n"
            assert-detect identity $ blank? "|\n "
            assert-detect not $ blank? |1
            assert-detect not $ blank? "| 1"
            assert-detect not $ blank? "|1 "

        |test-lisp-style $ quote
          fn ()
            log-title "|Test lisp style"

            assert=
              format-to-lisp $ quote (defn f1 (x) (+ x y))
              , "|(defn f1 (x) (+ x y))"

            assert=
              format-to-lisp $ quote $ nil? nil
              , "|(nil? nil)"

        |test-methods $ quote
          defn test-methods ()
            log-title "|Testing string methods"

            assert= true $ .blank? |
            assert= true $ .blank? "| "
            assert= false $ .blank? |a
            assert= 3 (.count |abc)
            assert= | (.empty |a)
            assert= true $ .ends-with? |abc |c
            assert= false $ .ends-with? |abc |b
            assert= |a $ .get |abc 0
            assert= |b $ .get |abc 1
            assert= 1 $ .parse-float |1
            assert= 1.1 $ .parse-float |1.1
            assert= |Abcd $ .replace |abcd |a |A
            assert= |AbAd $ .replace |abad |a |A

            assert=
              [] |a |c
              .split |abc |b
            assert=
              [] |a |c
              .split-lines "|a\nc"

            assert= true $ .starts-with? |abcd |a
            assert= false $ .starts-with? |abcd |b
            assert= |bcd $ .strip-prefix |abcd |a
            assert= |abc $ .strip-suffix |abcd |d
            assert= |abcd $ .strip-suffix |abcd |a
            assert= |bc $ .slice |abcd 1 3
            assert= |bcd $ .slice |abcd 1
            assert= |文字 $ .slice |中文字符串 1 3
            assert= |文字符串 $ .slice |中文字符串 1
            assert= "|ab cd" $ .trim "| ab cd"
            assert= true $ .empty? |
            assert= false $ .empty? "|a"
            assert= true $ .contains? |abcd 0
            assert= false $ .contains? |abcd 4
            assert= true $ .includes? |abcd |a
            assert= false $ .includes? |abcd |e
            assert= |a $ .nth |abc 0
            assert= |b $ .nth |abc 1
            assert= |a $ .first |abc
            assert= nil $ .first |
            assert= |bc $ .rest |abc
            assert= 0 $ .find-index |abc |a
            assert= 1 $ .find-index |abc |b
            assert= "|\"a \\\"\"" $ .escape "|a \""

        |main! $ quote
          defn main! ()
            log-title "|Testing str"
            test-str

            test-includes

            log-title "|Testing parse"
            test-parse

            log-title "|Testing trim"
            test-trim

            test-format

            test-char

            test-whitespace

            test-lisp-style

            test-methods

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
