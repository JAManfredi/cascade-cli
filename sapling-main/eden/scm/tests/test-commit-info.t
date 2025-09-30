Make sure various ways to mutate commits emit commit_info events.

  $ setconfig sampling.key.commit_info=commit-info
  $ setconfig sampling.filepath=$TESTTMP/samples
  $ setconfig sampling.debug=true

  $ enable absorb amend rebase

  $ newclientrepo
  $ drawdag <<EOS --config sampling.debug=false
  > B
  > |
  > A
  > EOS
  $ hg go -q $A
  $ echo foo > foo
  $ hg commit -Aqm foo
  {"category": "commit-info", "data": {"author": "test", "checkoutidentifier": "*", "metrics_type": "commit_info", "node": "25e0c68910a06abcb0d408bbb46d8a57d450303d", "repo": "repo1_server"}} (glob)
  $ hg amend -m bar
  {"category": "commit-info", "data": {"author": "test", "checkoutidentifier": "*", "metrics_type": "commit_info", "mutation": "amend", "node": "99a36353ec906250edc52a36dbabc9405a35f481", "predecessors": "25e0c68910a06abcb0d408bbb46d8a57d450303d", "repo": "repo1_server"}} (glob)
  $ hg metaedit -m foo
  {"category": "commit-info", "data": {"author": "test", "metrics_type": "commit_info", "mutation": "metaedit", "node": "52d9da81d33c4e6b4e3733b79f059e9ac5fb3e35", "predecessors": "99a36353ec906250edc52a36dbabc9405a35f481", "repo": "repo1_server"}}
  $ hg rebase -qr . -d $B
  {"category": "commit-info", "data": {"author": "test", "checkoutidentifier": "*", "metrics_type": "commit_info", "mutation": "rebase", "node": "1f942012ef43a9544aa16f4823d8453d7b75c410", "predecessors": "52d9da81d33c4e6b4e3733b79f059e9ac5fb3e35", "repo": "repo1_server"}} (glob)
  $ echo change > foo

FIXME: no "predecessors"
  $ hg absorb -qa
  {"category": "commit-info", "data": {"author": "test", "checkoutidentifier": "*", "metrics_type": "commit_info", "mutation": "absorb", "node": "7da347b1b542345cf23aecffbda02adc9ca8f08a", "predecessors": "1f942012ef43a9544aa16f4823d8453d7b75c410", "repo": "repo1_server"}} (glob)

  $ newclientrepo
  $ drawdag <<EOS --config sampling.debug=false
  > A
  > EOS

FIXME: no "predecessors"
  $ hg go -q $A
  $ hg debugimportstack << EOS
  > [["commit", {"author": "test", "date": [0, 0], "text": "changed", "mark": ":1", "parents": [],
  >   "predecessors": ["$A"], "files": {"A": {"data": "changed"}}}]]
  > EOS
  {"category": "commit-info", "data": {"author": "test", "metrics_type": "commit_info", "mutation": "importstack", "node": "75e5b2af7d86c003e77552934d7ce0da89d6a8a0", "predecessors": "426bada5c67598ca65036d57d9e4b64b0c1ce7a0", "repo": "repo2_server"}}
  {":1": "75e5b2af7d86c003e77552934d7ce0da89d6a8a0"}
